use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub count: i32,
    pub owner: Addr,
    pub dest_ic20_denom: String,
    pub dest_ic20_decimals: u8,
    pub src_ic20_denom: String,
    pub src_ic20_decimals: u8,
}

pub const STATE: Item<State> = Item::new("state");
