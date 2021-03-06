// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use std::convert::TryInto;

use common_arrow::arrow_flight::Action;
use common_exception::ErrorCode;
use common_exception::ToErrorCode;
use common_planners::Expression;
use common_planners::PlanNode;
use tonic::Status;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ShuffleAction {
    pub query_id: String,
    pub stage_id: String,
    pub plan: PlanNode,
    pub sinks: Vec<String>,
    pub scatters_expression: Expression,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct BroadcastAction {
    pub query_id: String,
    pub stage_id: String,
    pub plan: PlanNode,
    pub sinks: Vec<String>,
}

impl TryInto<ShuffleAction> for Vec<u8> {
    type Error = Status;

    fn try_into(self) -> Result<ShuffleAction, Self::Error> {
        match std::str::from_utf8(&self) {
            Err(cause) => Err(Status::invalid_argument(cause.to_string())),
            Ok(utf8_body) => match serde_json::from_str::<ShuffleAction>(utf8_body) {
                Err(cause) => Err(Status::invalid_argument(cause.to_string())),
                Ok(action) => Ok(action),
            },
        }
    }
}

impl TryInto<Vec<u8>> for ShuffleAction {
    type Error = ErrorCode;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        serde_json::to_vec(&self).map_err_to_code(ErrorCode::LogicalError, || {
            "Logical error: cannot serialize ShuffleAction."
        })
    }
}

impl TryInto<BroadcastAction> for Vec<u8> {
    type Error = Status;

    fn try_into(self) -> Result<BroadcastAction, Self::Error> {
        match std::str::from_utf8(&self) {
            Err(cause) => Err(Status::invalid_argument(cause.to_string())),
            Ok(utf8_body) => match serde_json::from_str::<BroadcastAction>(utf8_body) {
                Err(cause) => Err(Status::invalid_argument(cause.to_string())),
                Ok(action) => Ok(action),
            },
        }
    }
}

impl TryInto<Vec<u8>> for BroadcastAction {
    type Error = ErrorCode;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        serde_json::to_vec(&self).map_err_to_code(ErrorCode::LogicalError, || {
            "Logical error: cannot serialize BroadcastAction."
        })
    }
}

#[derive(Clone, Debug)]
pub enum FlightAction {
    PrepareShuffleAction(ShuffleAction),
    BroadcastAction(BroadcastAction),
}

impl FlightAction {
    pub fn get_query_id(&self) -> String {
        match self {
            FlightAction::BroadcastAction(action) => action.query_id.clone(),
            FlightAction::PrepareShuffleAction(action) => action.query_id.clone(),
        }
    }

    pub fn get_stage_id(&self) -> String {
        match self {
            FlightAction::BroadcastAction(action) => action.stage_id.clone(),
            FlightAction::PrepareShuffleAction(action) => action.stage_id.clone(),
        }
    }

    pub fn get_sinks(&self) -> Vec<String> {
        match self {
            FlightAction::BroadcastAction(action) => action.sinks.clone(),
            FlightAction::PrepareShuffleAction(action) => action.sinks.clone(),
        }
    }

    pub fn get_plan(&self) -> PlanNode {
        match self {
            FlightAction::BroadcastAction(action) => action.plan.clone(),
            FlightAction::PrepareShuffleAction(action) => action.plan.clone(),
        }
    }

    pub fn get_scatter_expression(&self) -> Option<Expression> {
        match self {
            FlightAction::BroadcastAction(_) => None,
            FlightAction::PrepareShuffleAction(action) => Some(action.scatters_expression.clone()),
        }
    }
}

impl TryInto<FlightAction> for Action {
    type Error = Status;

    fn try_into(self) -> Result<FlightAction, Self::Error> {
        match self.r#type.as_str() {
            "PrepareShuffleAction" => Ok(FlightAction::PrepareShuffleAction(self.body.try_into()?)),
            "BroadcastAction" => Ok(FlightAction::BroadcastAction(self.body.try_into()?)),
            un_implemented => Err(Status::unimplemented(format!(
                "UnImplement action {}",
                un_implemented
            ))),
        }
    }
}

impl TryInto<Action> for FlightAction {
    type Error = ErrorCode;

    fn try_into(self) -> Result<Action, Self::Error> {
        match self {
            FlightAction::PrepareShuffleAction(shuffle_action) => Ok(Action {
                r#type: String::from("PrepareShuffleAction"),
                body: shuffle_action.try_into()?,
            }),
            FlightAction::BroadcastAction(broadcast_action) => Ok(Action {
                r#type: String::from("BroadcastAction"),
                body: broadcast_action.try_into()?,
            }),
        }
    }
}
