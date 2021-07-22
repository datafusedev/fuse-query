// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.
//

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

use common_arrow::arrow::datatypes::Schema as ArrowSchema;
use common_arrow::arrow_flight;
use common_arrow::arrow_flight::FlightData;
use common_exception::ErrorCode;
use common_flights::meta_api_impl::CreateDatabaseAction;
use common_flights::meta_api_impl::CreateDatabaseActionResult;
use common_flights::meta_api_impl::CreateTableAction;
use common_flights::meta_api_impl::CreateTableActionResult;
use common_flights::meta_api_impl::DatabaseMeta;
use common_flights::meta_api_impl::DropDatabaseAction;
use common_flights::meta_api_impl::DropDatabaseActionResult;
use common_flights::meta_api_impl::DropTableAction;
use common_flights::meta_api_impl::DropTableActionResult;
use common_flights::meta_api_impl::GetDatabaseAction;
use common_flights::meta_api_impl::GetDatabaseActionResult;
use common_flights::meta_api_impl::GetDatabasesReq;
use common_flights::meta_api_impl::GetTableAction;
use common_flights::meta_api_impl::GetTableActionResult;
use common_metatypes::Database;
use common_metatypes::Table;
use log::info;

use crate::executor::action_handler::RequestHandler;
use crate::executor::ActionHandler;
use crate::meta_service::cmd::Cmd::CreateDatabase;
use crate::meta_service::cmd::Cmd::CreateTable;
use crate::meta_service::cmd::Cmd::DropDatabase;
use crate::meta_service::cmd::Cmd::DropTable;
use crate::meta_service::AppliedState;
use crate::meta_service::LogEntry;

// Db
#[async_trait::async_trait]
impl RequestHandler<CreateDatabaseAction> for ActionHandler {
    async fn handle(
        &self,
        act: CreateDatabaseAction,
    ) -> common_exception::Result<CreateDatabaseActionResult> {
        let plan = act.plan;
        let db_name = &plan.db;
        let if_not_exists = plan.if_not_exists;

        let cr = LogEntry {
            txid: None,
            cmd: CreateDatabase {
                name: db_name.clone(),
                if_not_exists,
                db: Database {
                    database_id: 0,
                    tables: HashMap::new(),
                },
            },
        };

        let rst = self
            .meta_node
            .write(cr)
            .await
            .map_err(|e| ErrorCode::MetaNodeInternalError(e.to_string()))?;

        match rst {
            AppliedState::DataBase { prev, result } => {
                if let Some(prev) = prev {
                    if if_not_exists {
                        Ok(CreateDatabaseActionResult {
                            database_id: prev.database_id,
                        })
                    } else {
                        Err(ErrorCode::DatabaseAlreadyExists(format!(
                            "{} database exists",
                            db_name
                        )))
                    }
                } else {
                    Ok(CreateDatabaseActionResult {
                        database_id: result.unwrap().database_id,
                    })
                }
            }

            _ => Err(ErrorCode::MetaNodeInternalError("not a Database result")),
        }
    }
}

#[async_trait::async_trait]
impl RequestHandler<GetDatabaseAction> for ActionHandler {
    async fn handle(
        &self,
        act: GetDatabaseAction,
    ) -> common_exception::Result<GetDatabaseActionResult> {
        let db_name = act.db;
        let db = self.meta_node.get_database(&db_name).await;

        match db {
            Some(db) => {
                let rst = GetDatabaseActionResult {
                    database_id: db.database_id,
                    db: db_name,
                };
                Ok(rst)
            }
            None => Err(ErrorCode::UnknownDatabase(db_name)),
        }
    }
}

#[async_trait::async_trait]
impl RequestHandler<DropDatabaseAction> for ActionHandler {
    async fn handle(
        &self,
        act: DropDatabaseAction,
    ) -> common_exception::Result<DropDatabaseActionResult> {
        let db_name = &act.plan.db;
        let if_exists = act.plan.if_exists;
        let cr = LogEntry {
            txid: None,
            cmd: DropDatabase {
                name: db_name.clone(),
            },
        };

        let rst = self
            .meta_node
            .write(cr)
            .await
            .map_err(|e| ErrorCode::MetaNodeInternalError(e.to_string()))?;

        match rst {
            AppliedState::DataBase { prev, .. } => {
                if prev.is_some() || if_exists {
                    Ok(DropDatabaseActionResult {})
                } else {
                    Err(ErrorCode::UnknownDatabase(format!(
                        "database not found: {:}",
                        db_name
                    )))
                }
            }
            _ => Err(ErrorCode::MetaNodeInternalError("not a Database result")),
        }
    }
}

// table
#[async_trait::async_trait]
impl RequestHandler<CreateTableAction> for ActionHandler {
    async fn handle(
        &self,
        act: CreateTableAction,
    ) -> common_exception::Result<CreateTableActionResult> {
        let plan = act.plan;
        let db_name = &plan.db;
        let table_name = &plan.table;
        let if_not_exists = plan.if_not_exists;

        info!("create table: {:}: {:?}", &db_name, &table_name);

        let options = common_arrow::arrow::ipc::writer::IpcWriteOptions::default();
        let flight_data: FlightData =
            arrow_flight::SchemaAsIpc::new(&plan.schema.to_arrow(), &options).into();

        let table = Table {
            table_id: 0,
            schema: flight_data.data_header,
            parts: Default::default(),
        };

        let cr = LogEntry {
            txid: None,
            cmd: CreateTable {
                db_name: db_name.clone(),
                table_name: table_name.clone(),
                if_not_exists,
                table,
            },
        };

        let rst = self
            .meta_node
            .write(cr)
            .await
            .map_err(|e| ErrorCode::MetaNodeInternalError(e.to_string()))?;

        match rst {
            AppliedState::Table { prev, result } => {
                if let Some(prev) = prev {
                    if if_not_exists {
                        Ok(CreateTableActionResult {
                            table_id: prev.table_id,
                        })
                    } else {
                        Err(ErrorCode::TableAlreadyExists(format!(
                            "table exists: {}",
                            table_name
                        )))
                    }
                } else {
                    Ok(CreateTableActionResult {
                        table_id: result.unwrap().table_id,
                    })
                }
            }
            _ => Err(ErrorCode::MetaNodeInternalError("not a Table result")),
        }
    }
}

#[async_trait::async_trait]
impl RequestHandler<DropTableAction> for ActionHandler {
    async fn handle(
        &self,
        act: DropTableAction,
    ) -> common_exception::Result<DropTableActionResult> {
        let db_name = &act.plan.db;
        let table_name = &act.plan.table;
        let if_exists = act.plan.if_exists;

        let cr = LogEntry {
            txid: None,
            cmd: DropTable {
                db_name: db_name.clone(),
                table_name: table_name.clone(),
                if_exists,
            },
        };

        let rst = self
            .meta_node
            .write(cr)
            .await
            .map_err(|e| ErrorCode::MetaNodeInternalError(e.to_string()))?;

        match rst {
            AppliedState::Table { prev, .. } => {
                if prev.is_some() || if_exists {
                    Ok(DropTableActionResult {})
                } else {
                    Err(ErrorCode::UnknownTable(format!(
                        "table not found: {:}",
                        table_name
                    )))
                }
            }
            _ => Err(ErrorCode::MetaNodeInternalError("not a Table result")),
        }
    }
}

#[async_trait::async_trait]
impl RequestHandler<GetTableAction> for ActionHandler {
    async fn handle(&self, act: GetTableAction) -> common_exception::Result<GetTableActionResult> {
        let db_name = &act.db;
        let table_name = &act.table;

        let db = self.meta_node.get_database(db_name).await.ok_or_else(|| {
            ErrorCode::UnknownDatabase(format!("get table: database not found {:}", db_name))
        })?;

        let table_id = db
            .tables
            .get(table_name)
            .ok_or_else(|| ErrorCode::UnknownTable(format!("table not found: {:}", table_name)))?;

        let result = self.meta_node.get_table(table_id).await;

        match result {
            Some(table) => {
                let arrow_schema = ArrowSchema::try_from(&FlightData {
                    data_header: table.schema,
                    ..Default::default()
                })
                .map_err(|e| {
                    ErrorCode::IllegalSchema(format!("invalid schema: {:}", e.to_string()))
                })?;
                let rst = GetTableActionResult {
                    table_id: table.table_id,
                    db: db_name.clone(),
                    name: table_name.clone(),
                    schema: Arc::new(arrow_schema.into()),
                };
                Ok(rst)
            }
            None => Err(ErrorCode::UnknownTable(table_name)),
        }
    }
}
#[async_trait::async_trait]
impl RequestHandler<GetDatabasesReq> for ActionHandler {
    async fn handle(&self, _act: GetDatabasesReq) -> common_exception::Result<DatabaseMeta> {
        // TODO, something need deeper thought:
        // - Sync the whole database is not practical
        //      instead we should sync operation logs(like doris) or differences (like TiDB)
        //
        // - DatabaseMeta should be versioned (globally)
        //    e.g. increase a global version number for each DDL (like TiDB)
        //    or metadata tagged with txn-id (logical timestamp), if we store meta data in KV
        //
        // - Computation layer do not need all the database meta all the time, care about the
        //   relations being processed only. Instead, UI frontend, like worksheets may need to
        //   access all the database meta (or information_schema).
        let databases = self.meta_node.get_databases().await;
        Ok(DatabaseMeta { databases })
    }
}
