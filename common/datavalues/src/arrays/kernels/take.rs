// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::mem;
use std::sync::Arc;

use common_arrow::arrow::array::Array;
use common_arrow::arrow::array::ArrayData;
use common_arrow::arrow::array::BooleanArray;
use common_arrow::arrow::array::PrimitiveArray;
use common_arrow::arrow::array::StringArray;
use common_arrow::arrow::array::StringBuilder;
use common_arrow::arrow::array::UInt32Array;
use common_arrow::arrow::buffer::MutableBuffer;
use common_arrow::arrow::datatypes::DataType as ArrowDataType;

use crate::arrays::IntoTakeRandom;
use crate::arrays::*;
use crate::*;

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk without nulls and arrow array as index.
pub unsafe fn take_no_null_primitive<T: DFNumericType>(
    arr: &PrimitiveArray<T>,
    indices: &UInt32Array,
) -> Arc<PrimitiveArray<T>> {
    assert_eq!(arr.null_count(), 0);

    let data_len = indices.len();
    let array_values = arr.values();
    let index_values = indices.values();

    let mut av = AlignedVec::<T::Native>::with_capacity_len_aligned(data_len);
    av.iter_mut()
        .zip(index_values.iter())
        .for_each(|(num, idx)| {
            let _ = &array_values;
            *num = *array_values.get_unchecked(*idx as usize);
        });

    let nulls = indices.data_ref().null_buffer().cloned();
    let arr = av.into_primitive_array::<T>(nulls);
    Arc::new(arr)
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk without nulls and an iterator as index.
pub unsafe fn take_no_null_primitive_iter_unchecked<
    T: DFNumericType,
    I: IntoIterator<Item = usize>,
>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    assert_eq!(arr.null_count(), 0);
    let indices_iter = indices.into_iter();
    let data_len = indices_iter.size_hint().0;
    let array_values = arr.values();

    let mut av = AlignedVec::<T::Native>::with_capacity_len_aligned(data_len);

    av.iter_mut().zip(indices_iter).for_each(|(num, idx)| {
        let _ = &array_values;
        *num = *array_values.get_unchecked(idx);
    });
    let arr = av.into_primitive_array::<T>(None);
    Arc::new(arr)
}

/// Take kernel for single chunk without nulls and an iterator as index that does bound checks.
pub fn take_no_null_primitive_iter<T: DFNumericType, I: IntoIterator<Item = usize>>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    assert_eq!(arr.null_count(), 0);

    let array_values = arr.values();

    let av = indices
        .into_iter()
        .map(|idx| {
            let _ = &array_values;
            array_values[idx]
        })
        .collect::<AlignedVec<_>>();
    let arr = av.into_primitive_array(None);

    Arc::new(arr)
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for a single chunk with null values and an iterator as index.
pub unsafe fn take_primitive_iter_unchecked<T: DFNumericType, I: IntoIterator<Item = usize>>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    let array_values = arr.values();

    let iter = indices.into_iter().map(|idx| {
        let _ = (&arr, &array_values);
        if arr.is_valid(idx) {
            Some(*array_values.get_unchecked(idx))
        } else {
            None
        }
    });
    let arr = PrimitiveArray::from_trusted_len_iter(iter);

    Arc::new(arr)
}

/// Take kernel for a single chunk with null values and an iterator as index that does bound checks.
pub fn take_primitive_iter<T: DFNumericType, I: IntoIterator<Item = usize>>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    let array_values = arr.values();

    let arr = indices
        .into_iter()
        .map(|idx| {
            let _ = (&arr, &array_values);
            if arr.is_valid(idx) {
                Some(array_values[idx])
            } else {
                None
            }
        })
        .collect();

    Arc::new(arr)
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for a single chunk without nulls and an iterator that can produce None values.
/// This is used in join operations.
pub unsafe fn take_no_null_primitive_opt_iter_unchecked<
    T: DFNumericType,
    I: IntoIterator<Item = Option<usize>>,
>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    let array_values = arr.values();

    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.map(|idx| {
            let _ = &array_values;
            *array_values.get_unchecked(idx)
        })
    });
    let arr = PrimitiveArray::from_trusted_len_iter(iter);

    Arc::new(arr)
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for a single chunk and an iterator that can produce None values.
/// This is used in join operations.
pub unsafe fn take_primitive_opt_iter_unchecked<
    T: DFNumericType,
    I: IntoIterator<Item = Option<usize>>,
>(
    arr: &PrimitiveArray<T>,
    indices: I,
) -> Arc<PrimitiveArray<T>> {
    let array_values = arr.values();

    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.and_then(|idx| {
            let _ = (&arr, &array_values);
            if arr.is_valid(idx) {
                Some(*array_values.get_unchecked(idx))
            } else {
                None
            }
        })
    });
    let arr = PrimitiveArray::from_trusted_len_iter(iter);

    Arc::new(arr)
}

/// Take kernel for multiple chunks. We directly return a DataArray because that path chooses the fastest collection path.
pub fn take_primitive_iter_n_arrays<T: DFNumericType, I: IntoIterator<Item = usize>>(
    ca: &DataArray<T>,
    indices: I,
) -> DataArray<T> {
    let taker = ca.take_rand();
    indices.into_iter().map(|idx| taker.get(idx)).collect()
}

/// Take kernel for multiple chunks where an iterator can produce None values.
/// Used in join operations. We directly return a DataArray because that path chooses the fastest collection path.
pub fn take_primitive_opt_iter_n_arrays<T: DFNumericType, I: IntoIterator<Item = Option<usize>>>(
    ca: &DataArray<T>,
    indices: I,
) -> DataArray<T> {
    let taker = ca.take_rand();
    indices
        .into_iter()
        .map(|opt_idx| opt_idx.and_then(|idx| taker.get(idx)))
        .collect()
}

/// Take kernel for single chunk without nulls and an iterator as index that does bound checks.
pub fn take_no_null_bool_iter<I: IntoIterator<Item = usize>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    debug_assert_eq!(arr.null_count(), 0);

    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        Some(arr.value(idx))
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk without nulls and an iterator as index.
pub unsafe fn take_no_null_bool_iter_unchecked<I: IntoIterator<Item = usize>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    debug_assert_eq!(arr.null_count(), 0);
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        Some(arr.value_unchecked(idx))
    });

    Arc::new(iter.collect())
}

/// Take kernel for single chunk and an iterator as index that does bound checks.
pub fn take_bool_iter<I: IntoIterator<Item = usize>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        if arr.is_null(idx) {
            None
        } else {
            Some(arr.value(idx))
        }
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk and an iterator as index.
pub unsafe fn take_bool_iter_unchecked<I: IntoIterator<Item = usize>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        if arr.is_null(idx) {
            None
        } else {
            Some(arr.value_unchecked(idx))
        }
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk and an iterator as index.
pub unsafe fn take_bool_opt_iter_unchecked<I: IntoIterator<Item = Option<usize>>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.and_then(|idx| {
            let _ = &arr;
            if arr.is_null(idx) {
                None
            } else {
                Some(arr.value_unchecked(idx))
            }
        })
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
/// Take kernel for single chunk without null values and an iterator as index that may produce None values.
pub unsafe fn take_no_null_bool_opt_iter_unchecked<I: IntoIterator<Item = Option<usize>>>(
    arr: &BooleanArray,
    indices: I,
) -> Arc<BooleanArray> {
    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.map(|idx| {
            let _ = &arr;
            arr.value_unchecked(idx)
        })
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
pub unsafe fn take_no_null_utf8_iter_unchecked<I: IntoIterator<Item = usize>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        Some(arr.value_unchecked(idx))
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
pub unsafe fn take_utf8_iter_unchecked<I: IntoIterator<Item = usize>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        if arr.is_null(idx) {
            None
        } else {
            Some(arr.value_unchecked(idx))
        }
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
pub unsafe fn take_no_null_utf8_opt_iter_unchecked<I: IntoIterator<Item = Option<usize>>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.map(|idx| {
            let _ = &arr;
            arr.value_unchecked(idx)
        })
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
pub unsafe fn take_utf8_opt_iter_unchecked<I: IntoIterator<Item = Option<usize>>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|opt_idx| {
        opt_idx.and_then(|idx| {
            let _ = &arr;
            if arr.is_null(idx) {
                None
            } else {
                Some(arr.value_unchecked(idx))
            }
        })
    });

    Arc::new(iter.collect())
}

pub fn take_no_null_utf8_iter<I: IntoIterator<Item = usize>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        Some(arr.value(idx))
    });

    Arc::new(iter.collect())
}

pub fn take_utf8_iter<I: IntoIterator<Item = usize>>(
    arr: &StringArray,
    indices: I,
) -> Arc<StringArray> {
    let iter = indices.into_iter().map(|idx| {
        let _ = &arr;
        if arr.is_null(idx) {
            None
        } else {
            Some(arr.value(idx))
        }
    });

    Arc::new(iter.collect())
}

/// # Safety
/// Note this doesn't do any bound checking, for performance reason.
pub unsafe fn take_utf8(arr: &StringArray, indices: &UInt32Array) -> Arc<StringArray> {
    let data_len = indices.len();

    let offset_len_in_bytes = (data_len + 1) * mem::size_of::<i64>();
    let mut offset_buf = MutableBuffer::new(offset_len_in_bytes);
    offset_buf.resize(offset_len_in_bytes, 0);
    let offset_typed = offset_buf.typed_data_mut();

    let mut length_so_far = 0;
    offset_typed[0] = length_so_far;

    let nulls;

    // The required size is yet unknown
    // Allocate 2.0 times the expected size.
    // where expected size is the length of bytes multiplied by the factor (take_len / current_len)
    let mut values_capacity = if arr.len() > 0 {
        ((arr.value_data().len() as f32 * 2.0) as usize) / arr.len() * indices.len() as usize
    } else {
        0
    };

    // 16 bytes per string as default alloc
    let mut values_buf = AlignedVec::<u8>::with_capacity_aligned(values_capacity);

    // both 0 nulls
    if arr.null_count() == 0 && indices.null_count() == 0 {
        offset_typed
            .iter_mut()
            .skip(1)
            .enumerate()
            .for_each(|(idx, offset)| {
                let _ = (&indices, &arr);
                let index = indices.value_unchecked(idx) as usize;
                let s = arr.value_unchecked(index);
                length_so_far += s.len() as i64;
                *offset = length_so_far;

                if length_so_far as usize >= values_capacity {
                    values_buf.reserve(values_capacity);
                    values_capacity *= 2;
                }

                values_buf.extend_from_slice(s.as_bytes())
            });
        nulls = None;
    } else if arr.null_count() == 0 {
        offset_typed
            .iter_mut()
            .skip(1)
            .enumerate()
            .for_each(|(idx, offset)| {
                let _ = (&indices, &arr);
                if indices.is_valid(idx) {
                    let index = indices.value_unchecked(idx) as usize;
                    let s = arr.value_unchecked(index);
                    length_so_far += s.len() as i64;

                    if length_so_far as usize >= values_capacity {
                        values_buf.reserve(values_capacity);
                        values_capacity *= 2;
                    }

                    values_buf.extend_from_slice(s.as_bytes())
                }
                *offset = length_so_far;
            });
        nulls = indices.data_ref().null_buffer().cloned();
    } else {
        let mut builder = StringBuilder::with_capacity(data_len, length_so_far as usize);

        if indices.null_count() == 0 {
            (0..data_len).for_each(|idx| {
                let _ = (&indices, &arr);
                let index = indices.value_unchecked(idx) as usize;
                if arr.is_valid(index) {
                    let s = arr.value_unchecked(index);
                    builder.append_value(s).unwrap();
                } else {
                    builder.append_null().unwrap();
                }
            });
        } else {
            (0..data_len).for_each(|idx| {
                let _ = (&indices, &arr);
                if indices.is_valid(idx) {
                    let index = indices.value_unchecked(idx) as usize;

                    if arr.is_valid(index) {
                        let s = arr.value_unchecked(index);
                        builder.append_value(s).unwrap();
                    } else {
                        builder.append_null().unwrap();
                    }
                } else {
                    builder.append_null().unwrap();
                }
            });
        }

        return Arc::new(builder.finish());
    }

    let mut data = ArrayData::builder(ArrowDataType::Utf8)
        .len(data_len)
        .add_buffer(offset_buf.into())
        .add_buffer(values_buf.into_arrow_buffer());
    if let Some(null_buffer) = nulls {
        data = data.null_bit_buffer(null_buffer);
    }
    Arc::new(StringArray::from(data.build()))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_utf8_kernel() {
        let s = StringArray::from(vec![Some("foo"), None, Some("bar")]);
        unsafe {
            let out = take_utf8(&s, &UInt32Array::from(vec![1, 2]));
            assert!(out.is_null(0));
            assert!(out.is_valid(1));
            let out = take_utf8(&s, &UInt32Array::from(vec![None, Some(2)]));
            assert!(out.is_null(0));
            assert!(out.is_valid(1));
            let out = take_utf8(&s, &UInt32Array::from(vec![None, None]));
            assert!(out.is_null(0));
            assert!(out.is_null(1));
        }
    }
}
