// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use common_exception::Result;
use common_infallible::RwLock;
use common_progress::Progress;
use common_runtime::Runtime;
use futures::future::AbortHandle;
use uuid::Uuid;

use crate::clusters::ClusterRef;
use crate::configs::Config;
use crate::datasources::DatabaseCatalog;
use crate::sessions::Session;
use crate::sessions::Settings;

/// Data that needs to be shared in a query context.
/// This is very useful, for example, for queries:
///     USE database_1;
///     SELECT
///         (SELECT scalar FROM table_name_1) AS scalar_1,
///         (SELECT scalar FROM table_name_2) AS scalar_2,
///         (SELECT scalar FROM table_name_3) AS scalar_3
///     FROM table_name_4;
/// For each subquery, they will share a runtime, session, progress, init_query_id
pub struct FuseQueryContextShared {
    pub(in crate::sessions) conf: Config,
    pub(in crate::sessions) progress: Arc<Progress>,
    pub(in crate::sessions) session: Arc<Session>,
    pub(in crate::sessions) runtime: Arc<RwLock<Option<Arc<Runtime>>>>,
    pub(in crate::sessions) init_query_id: Arc<RwLock<String>>,
    pub(in crate::sessions) cluster_cache: Arc<RwLock<Option<ClusterRef>>>,
    pub(in crate::sessions) sources_abort_handle: Arc<RwLock<Vec<AbortHandle>>>,
    pub(in crate::sessions) ref_count: Arc<AtomicUsize>,
    pub(in crate::sessions) subquery_index: Arc<AtomicUsize>,
    pub(in crate::sessions) running_query: Arc<RwLock<Option<String>>>,
}

impl FuseQueryContextShared {
    pub fn try_create(conf: Config, session: Arc<Session>) -> Arc<FuseQueryContextShared> {
        Arc::new(FuseQueryContextShared {
            conf,
            init_query_id: Arc::new(RwLock::new(Uuid::new_v4().to_string())),
            progress: Arc::new(Progress::create()),
            session,
            runtime: Arc::new(RwLock::new(None)),
            cluster_cache: Arc::new(RwLock::new(None)),
            sources_abort_handle: Arc::new(RwLock::new(Vec::new())),
            ref_count: Arc::new(AtomicUsize::new(0)),
            subquery_index: Arc::new(AtomicUsize::new(1)),
            running_query: Arc::new(RwLock::new(None)),
        })
    }

    pub fn kill(&self) {
        let mut sources_abort_handle = self.sources_abort_handle.write();

        while let Some(source_abort_handle) = sources_abort_handle.pop() {
            source_abort_handle.abort();
        }

        // TODO: Wait for the query to be processed (write out the last error)
    }

    pub fn try_get_cluster(&self) -> Result<ClusterRef> {
        // We only get the cluster once during the query.
        let mut cluster_cache = self.cluster_cache.write();

        match &*cluster_cache {
            Some(cached) => Ok(cached.clone()),
            None => {
                let cluster = self.session.try_get_cluster()?;
                *cluster_cache = Some(cluster.clone());
                Ok(cluster)
            }
        }
    }

    pub fn get_current_database(&self) -> String {
        self.session.get_current_database()
    }

    pub fn set_current_database(&self, new_database_name: String) {
        self.session.set_current_database(new_database_name);
    }

    pub fn get_settings(&self) -> Arc<Settings> {
        self.session.get_settings()
    }

    pub fn get_datasource(&self) -> Arc<DatabaseCatalog> {
        self.session.get_datasource()
    }

    /// Init runtime when first get
    pub fn try_get_runtime(&self) -> Result<Arc<Runtime>> {
        let mut query_runtime = self.runtime.write();

        match &*query_runtime {
            Some(query_runtime) => Ok(query_runtime.clone()),
            None => {
                let settings = self.get_settings();
                let max_threads = settings.get_max_threads()? as usize;
                let runtime = Arc::new(Runtime::with_worker_threads(max_threads)?);
                *query_runtime = Some(runtime.clone());
                Ok(runtime)
            }
        }
    }

    pub fn attach_query_info(&self, query: &str) {
        let mut running_query = self.running_query.write();
        *running_query = Some(query.to_string());
    }

    pub fn add_source_abort_handle(&self, handle: AbortHandle) {
        let mut sources_abort_handle = self.sources_abort_handle.write();
        sources_abort_handle.push(handle);
    }
}

impl Session {
    pub(in crate::sessions) fn destroy_context_shared(&self) {
        let mut mutable_state = self.mutable_state.lock();
        mutable_state.context_shared.take();
    }
}
