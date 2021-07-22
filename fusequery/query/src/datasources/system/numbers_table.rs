// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::any::Any;
use std::mem::size_of;
use std::sync::Arc;

use common_datavalues::DataField;
use common_datavalues::DataSchemaRef;
use common_datavalues::DataSchemaRefExt;
use common_datavalues::DataType;
use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::Expression;
use common_planners::ReadDataSourcePlan;
use common_planners::ScanPlan;
use common_planners::Statistics;
use common_streams::SendableDataBlockStream;

use crate::datasources::system::NumbersStream;
use crate::datasources::Common;
use crate::datasources::Table;
use crate::datasources::TableFunction;
use crate::sessions::FuseQueryContextRef;

pub struct NumbersTable {
    table: &'static str,
    schema: DataSchemaRef,
}

impl NumbersTable {
    pub fn create(table: &'static str) -> Self {
        NumbersTable {
            table,
            schema: DataSchemaRefExt::create(vec![DataField::new(
                "number",
                DataType::UInt64,
                false,
            )]),
        }
    }
}

#[async_trait::async_trait]
impl Table for NumbersTable {
    fn name(&self) -> &str {
        self.table
    }

    fn engine(&self) -> &str {
        match self.table {
            "numbers" => "SystemNumbers",
            "numbers_mt" => "SystemNumbersMt",
            "numbers_local" => "SystemNumbersLocal",
            _ => unreachable!(),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> Result<DataSchemaRef> {
        Ok(self.schema.clone())
    }

    // As remote for performance test.
    fn is_local(&self) -> bool {
        self.table == "numbers_local"
    }

    fn read_plan(
        &self,
        ctx: FuseQueryContextRef,
        scan: &ScanPlan,
        _partitions: usize,
    ) -> Result<ReadDataSourcePlan> {
        let mut total = None;
        let ScanPlan { table_args, .. } = scan.clone();
        if let Some(Expression::Literal { value, .. }) = table_args {
            total = Some(value.as_u64()?);
        }

        let total = total.ok_or_else(|| {
            let _ = &self;
            ErrorCode::BadArguments(format!(
                "Must have one number argument for table: system.{}",
                self.name()
            ))
        })?;

        let statistics =
            Statistics::new_exact(total as usize, ((total) * size_of::<u64>() as u64) as usize);
        ctx.try_set_statistics(&statistics)?;
        ctx.add_total_rows_approx(statistics.read_rows);

        Ok(ReadDataSourcePlan {
            db: "system".to_string(),
            table: self.name().to_string(),
            schema: self.schema.clone(),
            parts: Common::generate_parts(0, ctx.get_settings().get_max_threads()?, total),
            statistics: statistics.clone(),
            description: format!(
                "(Read from system.{} table, Read Rows:{}, Read Bytes:{})",
                self.table, statistics.read_rows, statistics.read_bytes
            ),
            scan_plan: Arc::new(scan.clone()),
            remote: false,
        })
    }

    async fn read(
        &self,
        ctx: FuseQueryContextRef,
        _source_plan: &ReadDataSourcePlan,
    ) -> Result<SendableDataBlockStream> {
        Ok(Box::pin(NumbersStream::try_create(
            ctx,
            self.schema.clone(),
        )?))
    }
}

impl TableFunction for NumbersTable {
    fn function_name(&self) -> &str {
        self.table
    }

    fn db(&self) -> &str {
        "system"
    }

    fn as_table<'a>(self: Arc<Self>) -> Arc<dyn Table + 'a>
    where Self: 'a {
        self
    }
}
