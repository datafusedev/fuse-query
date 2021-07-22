// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::sync::Arc;

use common_datavalues::DataSchemaRef;
use common_datavalues::DataValue;
use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::AggregatorFinalPlan;
use common_planners::AggregatorPartialPlan;
use common_planners::BroadcastPlan;
use common_planners::Expression;
use common_planners::LimitByPlan;
use common_planners::LimitPlan;
use common_planners::PlanBuilder;
use common_planners::PlanNode;
use common_planners::PlanRewriter;
use common_planners::ReadDataSourcePlan;
use common_planners::SortPlan;
use common_planners::StageKind;
use common_planners::StagePlan;

use crate::optimizers::Optimizer;
use crate::sessions::FuseQueryContext;
use crate::sessions::FuseQueryContextRef;

pub struct ScattersOptimizer {
    ctx: FuseQueryContextRef,
}

#[derive(Clone, Debug)]
enum RunningMode {
    Standalone,
    Cluster,
}

struct ScattersOptimizerImpl {
    ctx: FuseQueryContextRef,
    running_mode: RunningMode,
    before_group_by_schema: Option<DataSchemaRef>,

    // temporary node
    input: Option<Arc<PlanNode>>,
}

impl ScattersOptimizerImpl {
    pub fn create(ctx: FuseQueryContextRef) -> ScattersOptimizerImpl {
        ScattersOptimizerImpl {
            ctx,
            running_mode: RunningMode::Standalone,
            before_group_by_schema: None,
            input: None,
        }
    }

    fn cluster_aggregate_without_key(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        // If no group by we convergent it in local node
        self.running_mode = RunningMode::Standalone;

        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Cluster aggr input is None")),
            Some(input) => Self::convergent_shuffle_stage(
                PlanBuilder::from(input.as_ref())
                    .aggregate_partial(&plan.aggr_expr, &plan.group_expr)?
                    .build()?,
            ),
        }
    }

    fn cluster_aggregate_with_key(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        // Keep running in cluster mode
        self.running_mode = RunningMode::Cluster;

        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Cluster aggr input is None")),
            Some(input) => Self::normal_shuffle_stage(
                "_group_by_key",
                PlanBuilder::from(input.as_ref())
                    .aggregate_partial(&plan.aggr_expr, &plan.group_expr)?
                    .build()?,
            ),
        }
    }

    fn cluster_aggregate(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        match plan.group_expr.len() {
            0 => self.cluster_aggregate_without_key(plan),
            _ => self.cluster_aggregate_with_key(plan),
        }
    }

    fn standalone_aggregate(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Standalone aggr input is None")),
            Some(input) => PlanBuilder::from(input.as_ref())
                .aggregate_partial(&plan.aggr_expr, &plan.group_expr)?
                .build(),
        }
    }

    fn cluster_sort(&mut self, plan: &SortPlan) -> Result<PlanNode> {
        // Order by we convergent it in local node
        self.running_mode = RunningMode::Standalone;

        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Cluster sort input is None")),
            Some(input) => Self::convergent_shuffle_stage_builder(input)
                .sort(&plan.order_by)?
                .build(),
        }
    }

    fn standalone_sort(&mut self, plan: &SortPlan) -> Result<PlanNode> {
        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Standalone sort input is None")),
            Some(input) => PlanBuilder::from(input.as_ref())
                .sort(&plan.order_by)?
                .build(),
        }
    }

    fn cluster_limit(&mut self, plan: &LimitPlan) -> Result<PlanNode> {
        // Limit we convergent it in local node
        self.running_mode = RunningMode::Standalone;

        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Cluster limit input is None")),
            Some(input) => Self::convergent_shuffle_stage_builder(input)
                .limit_offset(plan.n, plan.offset)?
                .build(),
        }
    }

    fn standalone_limit(&mut self, plan: &LimitPlan) -> Result<PlanNode> {
        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Standalone limit input is None")),
            Some(input) => PlanBuilder::from(input.as_ref())
                .limit_offset(plan.n, plan.offset)?
                .build(),
        }
    }

    fn cluster_limit_by(&mut self, plan: &LimitByPlan) -> Result<PlanNode> {
        // Limit by we convergent it in local node
        self.running_mode = RunningMode::Standalone;

        match self.input.take() {
            None => Err(ErrorCode::LogicalError("Cluster limit by input is None.")),
            Some(input) => Self::convergent_shuffle_stage_builder(input)
                .limit_by(plan.limit, &plan.limit_by)?
                .build(),
        }
    }

    fn standalone_limit_by(&mut self, plan: &LimitByPlan) -> Result<PlanNode> {
        match self.input.take() {
            None => Err(ErrorCode::LogicalError(
                "Standalone limit by input is None.",
            )),
            Some(input) => PlanBuilder::from(input.as_ref())
                .limit_by(plan.limit, &plan.limit_by)?
                .build(),
        }
    }

    fn convergent_shuffle_stage_builder(input: Arc<PlanNode>) -> PlanBuilder {
        PlanBuilder::from(&PlanNode::Stage(StagePlan {
            kind: StageKind::Convergent,
            scatters_expr: Expression::create_literal(DataValue::UInt64(Some(0))),
            input,
        }))
    }

    fn convergent_shuffle_stage(input: PlanNode) -> Result<PlanNode> {
        Ok(PlanNode::Stage(StagePlan {
            kind: StageKind::Convergent,
            scatters_expr: Expression::create_literal(DataValue::UInt64(Some(0))),
            input: Arc::new(input),
        }))
    }

    fn normal_shuffle_stage(key: impl Into<String>, input: PlanNode) -> Result<PlanNode> {
        let scatters_expr = Expression::ScalarFunction {
            op: String::from("sipHash"),
            args: vec![Expression::Column(key.into())],
        };

        Ok(PlanNode::Stage(StagePlan {
            scatters_expr,
            kind: StageKind::Normal,
            input: Arc::new(input),
        }))
    }
}

impl PlanRewriter for ScattersOptimizerImpl {
    fn rewrite_subquery_plan(&mut self, subquery_plan: &PlanNode) -> Result<PlanNode> {
        let subquery_ctx = FuseQueryContext::new(self.ctx.clone());
        let mut subquery_optimizer = ScattersOptimizerImpl::create(subquery_ctx);
        let rewritten_subquery = subquery_optimizer.rewrite_plan_node(subquery_plan)?;

        match (&self.running_mode, &subquery_optimizer.running_mode) {
            (RunningMode::Standalone, RunningMode::Standalone) => Ok(rewritten_subquery),
            (RunningMode::Standalone, RunningMode::Cluster) => {
                Ok(Self::convergent_shuffle_stage(rewritten_subquery)?)
            }
            (RunningMode::Cluster, RunningMode::Standalone) => {
                Ok(PlanNode::Broadcast(BroadcastPlan {
                    input: Arc::new(rewritten_subquery),
                }))
            }
            (RunningMode::Cluster, RunningMode::Cluster) => {
                Ok(PlanNode::Broadcast(BroadcastPlan {
                    input: Arc::new(rewritten_subquery),
                }))
            }
        }
    }

    fn rewrite_aggregate_partial(&mut self, plan: &AggregatorPartialPlan) -> Result<PlanNode> {
        let new_input = Arc::new(self.rewrite_plan_node(&plan.input)?);

        self.input = Some(new_input.clone());
        self.before_group_by_schema = Some(new_input.schema());

        match self.running_mode {
            RunningMode::Cluster => self.cluster_aggregate(plan),
            RunningMode::Standalone => self.standalone_aggregate(plan),
        }
    }

    fn rewrite_aggregate_final(&mut self, plan: &AggregatorFinalPlan) -> Result<PlanNode> {
        let new_input = self.rewrite_plan_node(&plan.input)?;

        match self.before_group_by_schema.take() {
            None => Ok(PlanNode::AggregatorFinal(plan.clone())),
            Some(schema_before_group_by) => PlanBuilder::from(&new_input)
                .aggregate_final(schema_before_group_by, &plan.aggr_expr, &plan.group_expr)?
                .build(),
        }
    }

    fn rewrite_sort(&mut self, plan: &SortPlan) -> Result<PlanNode> {
        self.input = Some(Arc::new(self.rewrite_plan_node(plan.input.as_ref())?));

        match self.running_mode {
            RunningMode::Cluster => self.cluster_sort(plan),
            RunningMode::Standalone => self.standalone_sort(plan),
        }
    }

    fn rewrite_limit(&mut self, plan: &LimitPlan) -> Result<PlanNode> {
        self.input = Some(Arc::new(self.rewrite_plan_node(plan.input.as_ref())?));

        match self.running_mode {
            RunningMode::Cluster => self.cluster_limit(plan),
            RunningMode::Standalone => self.standalone_limit(plan),
        }
    }

    fn rewrite_limit_by(&mut self, plan: &LimitByPlan) -> Result<PlanNode> {
        self.input = Some(Arc::new(self.rewrite_plan_node(plan.input.as_ref())?));

        match self.running_mode {
            RunningMode::Cluster => self.cluster_limit_by(plan),
            RunningMode::Standalone => self.standalone_limit_by(plan),
        }
    }

    fn rewrite_read_data_source(&mut self, plan: &ReadDataSourcePlan) -> Result<PlanNode> {
        let context = self.ctx.clone();
        let select_table = context.get_table(&plan.db, &plan.table)?;

        match select_table.get_inner().is_local() {
            false => self.running_mode = RunningMode::Cluster,
            true => self.running_mode = RunningMode::Standalone,
        }

        Ok(PlanNode::ReadSource(plan.clone()))
    }
}

impl ScattersOptimizer {
    pub fn create(ctx: FuseQueryContextRef) -> ScattersOptimizer {
        ScattersOptimizer { ctx }
    }
}

impl Optimizer for ScattersOptimizer {
    fn name(&self) -> &str {
        "Scatters"
    }

    fn optimize(&mut self, plan: &PlanNode) -> Result<PlanNode> {
        if self.ctx.try_get_cluster()?.is_empty()? {
            // Standalone mode.
            return Ok(plan.clone());
        }

        let mut optimizer_impl = ScattersOptimizerImpl::create(self.ctx.clone());
        let rewrite_plan = optimizer_impl.rewrite_plan_node(plan)?;

        // We need to converge at the end
        match optimizer_impl.running_mode {
            RunningMode::Standalone => Ok(rewrite_plan),
            RunningMode::Cluster => Ok(PlanNode::Stage(StagePlan {
                kind: StageKind::Convergent,
                scatters_expr: Expression::create_literal(DataValue::UInt64(Some(0))),
                input: Arc::new(rewrite_plan),
            })),
        }
    }
}
