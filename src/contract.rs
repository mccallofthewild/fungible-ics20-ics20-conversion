#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
    Uint128, Uint256,
};
use cw2::set_contract_version;
use cw20::{Denom, Expiration};

use crate::error::ContractError;
use crate::msg::{ConvertTokenResponse, CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{State, STATE};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:fungible-ics20-ics20-conversion";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let state = State {
        count: msg.count,
        owner: info.sender.clone(),
        dest_ic20_decimals: msg.dest_ic20_decimals.clone(),
        dest_ic20_denom: msg.dest_ic20_denom.clone(),
        src_ic20_decimals: msg.src_ic20_decimals.clone(),
        src_ic20_denom: msg.src_ic20_denom.clone(),
    };
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("count", msg.count.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Increment {} => try_increment(deps),
        ExecuteMsg::Reset { count } => try_reset(deps, info, count),
    }
}

pub fn deposit_dest_tokens(
    deps: DepsMut,
    info: &MessageInfo,
    _env: Env,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    if !info.funds.iter().all(|f| f.denom == state.dest_ic20_denom) {
        return Err(ContractError::InvalidFunds {});
    }
    return Ok(Response::new());
}

pub fn convert_tokens(
    deps: DepsMut,
    info: &MessageInfo,
    _env: Env,
    src_token_amount: Uint128,
) -> Result<Response, ContractError> {
    let state = STATE.load(deps.storage)?;
    let src_denom = state.src_ic20_denom.clone();
    // make sure it's the right token and count how much has been sent.
    if !info.funds.iter().all(|f| f.denom == state.dest_ic20_denom) {
        return Err(ContractError::InvalidFunds {});
    }
    let received_src_token_amount: Uint128 = info
        .funds
        .iter()
        .filter(|c| c.denom == src_denom)
        .map(|c| c.amount)
        .sum();
    if received_src_token_amount != src_token_amount {
        return Err(ContractError::InvalidFunds {});
    }

    let out_token_amount = calculate_token_conversion_output(
        received_src_token_amount.u128(),
        10 * *&(state.dest_ic20_decimals.clone() as u128),
        state.src_ic20_decimals.clone(),
        state.dest_ic20_decimals.clone(),
    )?;
    // convert the sent amount to the destination token denomination & decimals

    let transfer_msg = get_bank_transfer_to_msg(
        &info.sender,
        &state.dest_ic20_denom.clone(),
        Uint128::from(out_token_amount.amount.clone()),
    );
    Ok(Response::new().add_message(transfer_msg))
}

/// Convert between tokens with different decimals.
///
/// # Arguments
///
/// * `amount` - the amount of the input token to convert
/// * `rate` - corresponds to the output token decimals. E.g: If we want 1:1 rate and the output token has 6 decimals, then rate = 1_000_000
/// * `input_decimals` - the number of decimals of the input token
/// * `output_decimals` - the number of decimals of the output token
pub fn calculate_token_conversion_output(
    amount: u128,
    rate: u128,
    input_decimals: u8,
    output_decimals: u8,
) -> StdResult<ConvertTokenResponse> {
    // result = amount * rate / one whole output token
    let mut result = amount * rate;

    // But, if tokens have different number of decimals, we need to compensate either by
    // dividing or multiplying (depending on which token has more decimals) the difference
    if input_decimals < output_decimals {
        let compensation = get_whole_token_representation(output_decimals - input_decimals);
        result = result * compensation
    } else if output_decimals < input_decimals {
        let compensation = get_whole_token_representation(input_decimals - output_decimals);
        result = result / compensation
    }

    let whole_token = get_whole_token_representation(output_decimals);

    let result = result / whole_token;

    Ok(ConvertTokenResponse { amount: result })
}

/// Get the amount needed to represent 1 whole token given its decimals.
/// Ex. Given token A that has 3 decimals, 1 A == 1000
pub fn get_whole_token_representation(decimals: u8) -> u128 {
    let mut whole_token = 1u128;

    for _ in 0..decimals {
        whole_token *= 10;
    }

    whole_token
}

fn get_bank_transfer_to_msg(recipient: &Addr, denom: &str, native_amount: Uint128) -> CosmosMsg {
    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: recipient.into(),
        amount: vec![Coin {
            denom: denom.to_string(),
            amount: native_amount,
        }],
    };

    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();
    transfer_bank_cosmos_msg
}

pub fn try_increment(deps: DepsMut) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        state.count += 1;
        Ok(state)
    })?;

    Ok(Response::new().add_attribute("method", "try_increment"))
}
pub fn try_reset(deps: DepsMut, info: MessageInfo, count: i32) -> Result<Response, ContractError> {
    STATE.update(deps.storage, |mut state| -> Result<_, ContractError> {
        if info.sender != state.owner {
            return Err(ContractError::Unauthorized {});
        }
        state.count = count;
        Ok(state)
    })?;
    Ok(Response::new().add_attribute("method", "reset"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => to_binary(&query_count(deps)?),
    }
}

fn query_count(deps: Deps) -> StdResult<CountResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(CountResponse { count: state.count })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            count: 17,
            src_ic20_decimals: 18,
            src_ic20_denom: "erc20token".to_string(),
            dest_ic20_decimals: 6,
            dest_ic20_denom: "cosmostoken".to_string(),
        };
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(17, value.count);
    }

    #[test]
    fn increment() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            count: 17,
            src_ic20_decimals: 18,
            src_ic20_denom: "erc20token".to_string(),
            dest_ic20_decimals: 6,
            dest_ic20_denom: "cosmostoken".to_string(),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Increment {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // should increase counter by 1
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(18, value.count);
    }

    #[test]
    fn reset() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        let msg = InstantiateMsg {
            count: 17,
            src_ic20_decimals: 18,
            src_ic20_denom: "erc20token".to_string(),
            dest_ic20_decimals: 6,
            dest_ic20_denom: "cosmostoken".to_string(),
        };
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // beneficiary can release it
        let unauth_info = mock_info("anyone", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let res = execute(deps.as_mut(), mock_env(), unauth_info, msg);
        match res {
            Err(ContractError::Unauthorized {}) => {}
            _ => panic!("Must return unauthorized error"),
        }

        // only the original creator can reset the counter
        let auth_info = mock_info("creator", &coins(2, "token"));
        let msg = ExecuteMsg::Reset { count: 5 };
        let _res = execute(deps.as_mut(), mock_env(), auth_info, msg).unwrap();

        // should now be 5
        let res = query(deps.as_ref(), mock_env(), QueryMsg::GetCount {}).unwrap();
        let value: CountResponse = from_binary(&res).unwrap();
        assert_eq!(5, value.count);
    }
    #[test]
    fn test_convert_token() {
        // Assuming the user friendly (in the UI) exchange rate has been set to
        // 1 swapped_token (9 decimals) == 1.5 input_token (9 decimals):
        // the rate would be 1 / 1.5 = 0.(6) or 666666666 (0.(6) ** 10 * 9)
        // meaning the price for 1 whole swapped_token is
        // 1500000000 (1.5 * 10 ** 9 decimals) of input_token.

        // If we want to get 2 of swapped_token, we need to send 3 input_token
        // i.e. amount = 3000000000 (3 * 10 ** 9 decimals)

        let rate = 666_666_666;
        let amount = 3_000_000_000;

        let result = calculate_token_conversion_output(amount, rate, 9, 9).unwrap();
        assert_eq!(result.amount, 1_999_999_998);

        // Should work the same even if input_token has less decimals (ex. 6)
        // Here amount has 3 zeroes less because input_token now has 6 decimals, so
        // 1 input_token = 3000000 (3 * 10 ** 6)

        let rate = 666_666_666;
        let amount = 3_000_000;

        let result = calculate_token_conversion_output(amount, rate, 6, 9).unwrap();
        assert_eq!(result.amount, 1_999_999_998);

        // And the other way around - when swap_token has 6 decimals.
        // Here the rate and result have 3 less digits - to account for the less decimals

        let rate = 666_666;
        let amount = 3_000_000_000;

        let result = calculate_token_conversion_output(amount, rate, 9, 6).unwrap();
        assert_eq!(result.amount, 1_999_998);

        // erc20 to ics20 standard conversion test

        let rate = 1_000_000;
        let amount = 3_000_000_000_000_000_000;

        let result = calculate_token_conversion_output(amount, rate, 18, 6).unwrap();
        assert_eq!(result.amount, 3_000_000);
    }
}
