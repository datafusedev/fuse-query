// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::any::Any;
use std::convert::TryInto;
use std::fmt;

use common_datavalues::prelude::*;
use common_exception::Result;

use crate::aggregates::aggregator_common::assert_binary_arguments;
use crate::aggregates::AggregateFunction;
use crate::aggregates::AggregateSingeValueState;
use crate::aggregates::GetState;
use crate::aggregates::StateAddr;

#[derive(Clone)]
pub struct AggregateArgMaxFunction {
    display_name: String,
    arguments: Vec<DataField>,
}

impl AggregateArgMaxFunction {
    pub fn try_create(
        display_name: &str,
        arguments: Vec<DataField>,
    ) -> Result<Arc<dyn AggregateFunction>> {
        assert_binary_arguments(display_name, arguments.len())?;

        Ok(Arc::new(AggregateArgMaxFunction {
            display_name: display_name.to_string(),
            arguments,
        }))
    }
}

impl AggregateFunction for AggregateArgMaxFunction {
    fn name(&self) -> &str {
        "AggregateArgMaxFunction"
    }

    fn return_type(&self) -> Result<DataType> {
        Ok(self.arguments[0].data_type().clone())
    }

    fn nullable(&self, _input_schema: &DataSchema) -> Result<bool> {
        Ok(false)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn allocate_state(&self, arena: &bumpalo::Bump) -> StateAddr {
        let state = arena.alloc(AggregateSingeValueState {
            value: DataValue::Struct(vec![
                DataValue::from(self.arguments[0].data_type()),
                DataValue::from(self.arguments[1].data_type()),
            ]),
        });

        (state as *mut AggregateSingeValueState) as StateAddr
    }

    fn accumulate(
        &self,
        place: StateAddr,
        columns: &[DataColumn],
        _input_rows: usize,
    ) -> Result<()> {
        if columns[0].is_empty() {
            return Ok(());
        }

        let value = match &columns[1] {
            DataColumn::Constant(value, _) => Ok(DataValue::Struct(vec![
                DataValue::UInt64(Some(0)),
                value.clone(),
            ])),
            DataColumn::Array(array) => array.arg_max(),
        }?;

        if let DataValue::Struct(max_arg_val) = value {
            if max_arg_val[0].is_null() {
                return Ok(());
            }
            let index: u64 = max_arg_val[0].clone().try_into()?;
            let max_val = max_arg_val[1].clone();
            let max_arg = columns[0].try_get(index as usize)?;

            let state = AggregateSingeValueState::get(place);

            if let DataValue::Struct(old_max_arg_val) = state.value.clone() {
                let old_max_arg = old_max_arg_val[0].clone();
                let old_max_val = old_max_arg_val[1].clone();

                let new_max_val = DataValue::agg(Max, old_max_val.clone(), max_val)?;

                state.value = DataValue::Struct(vec![
                    if new_max_val == old_max_val {
                        old_max_arg
                    } else {
                        max_arg
                    },
                    new_max_val,
                ]);
            }
        }

        Ok(())
    }

    fn accumulate_row(&self, place: StateAddr, row: usize, columns: &[DataColumn]) -> Result<()> {
        let state = AggregateSingeValueState::get(place);

        if let DataValue::Struct(old_max_arg_val) = state.value.clone() {
            let old_max_arg = old_max_arg_val[0].clone();
            let old_max_val = old_max_arg_val[1].clone();

            let new_max_val = DataValue::agg(Max, old_max_val.clone(), columns[1].try_get(row)?)?;

            state.value = DataValue::Struct(vec![
                if new_max_val == old_max_val {
                    old_max_arg
                } else {
                    columns[0].try_get(row)?
                },
                new_max_val,
            ]);
        }
        Ok(())
    }

    fn serialize(&self, place: StateAddr, writer: &mut Vec<u8>) -> Result<()> {
        let state = AggregateSingeValueState::get(place);
        state.serialize(writer)
    }

    fn deserialize(&self, place: StateAddr, reader: &[u8]) -> Result<()> {
        let state = AggregateSingeValueState::get(place);
        state.deserialize(reader)
    }

    fn merge(&self, place: StateAddr, rhs: StateAddr) -> Result<()> {
        let state = AggregateSingeValueState::get(place);
        let rhs = AggregateSingeValueState::get(rhs);

        if let (DataValue::Struct(current), DataValue::Struct(other)) =
            (state.value.clone(), rhs.value.clone())
        {
            let new_max_val = DataValue::agg(Max, current[1].clone(), other[1].clone())?;

            state.value = DataValue::Struct(vec![
                if new_max_val == other[1] {
                    other[0].clone()
                } else {
                    current[0].clone()
                },
                new_max_val,
            ]);
        }
        Ok(())
    }

    fn merge_result(&self, place: StateAddr) -> Result<DataValue> {
        let state = AggregateSingeValueState::get(place);
        Ok(if let DataValue::Struct(state) = state.value.clone() {
            state[0].clone()
        } else {
            state.value.clone()
        })
    }
}

impl fmt::Display for AggregateArgMaxFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.display_name)
    }
}
