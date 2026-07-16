use alloy::rpc::types::{Filter, Log};
use alloy::primitives::{Address,keccak256, U256, FixedBytes};
use alloy::eips::BlockNumberOrTag;
use crate::cache::{BorrowPosition,MarketCache};



  fn read_address(topic: &alloy::primitives::B256) -> Address {
    Address::from_slice(&topic.as_slice()[12..])
}

fn read_u256(data: &[u8], offset: usize) -> U256 {
    U256::from_be_slice(&data[offset..offset + 32])
}


impl MarketCache {
   

   pub fn process_log(&self, log: &Log) {
    let Some(topic0) = log.topics().first() else { return };

    match topic0 {
        x if *x == keccak256("Borrow(bytes32,address,address,address,uint256,uint256)") => {
            self.update_borrow(log);
        }
        x if *x == keccak256("Repay(bytes32,address,address,uint256,uint256)") => {
            self.update_repay(log);
        }
        x if *x == keccak256("Liquidate(bytes32,address,address,uint256,uint256,uint256,uint256,uint256)") => {
            self.update_liquidate(log);
        }
        x if *x == keccak256("AccrueInterest(bytes32,uint256,uint256,uint256)") => {
            self.update_accrue_interest(log);
        }
        x if *x == keccak256("SupplyCollateral(bytes32,address,address,uint256)") => {
            self.update_supply_collateral(log);
        }
        x if *x == keccak256("WithdrawCollateral(bytes32,address,address,address,uint256)") => {
    self.update_withdraw_collateral(log);
}
        _ => {}
    }
}


/*  event Borrow(
        Id indexed id,
        address caller,
        address indexed onBehalf,
        address indexed receiver,
        uint256 assets,
        uint256 shares
    );
*/
pub fn update_borrow(&self, log: &Log) {
    let market_id = FixedBytes::from(log.topics()[1]);
    let on_behalf = read_address(&log.topics()[2]);
    let shares = read_u256(&log.data().data, 64);
    self.update(market_id, |m| {
        if let Some(pos) = m.positions.iter_mut().find(|p| p.address == on_behalf) {
            pos.borrow_shares += shares;
        }
    });
}

/*   event Repay(Id indexed id, address indexed caller, address indexed onBehalf, uint256 assets, uint256 shares); */
pub fn update_repay(&self, log: &Log) {
    let market_id = FixedBytes::from(log.topics()[1]);
    let on_behalf = read_address(&log.topics()[3]);
    let shares = read_u256(&log.data().data, 32);
    self.update(market_id, |m| {
        if let Some(pos) = m.positions.iter_mut().find(|p| p.address == on_behalf) {
            pos.borrow_shares = pos.borrow_shares.saturating_sub(shares);
        }
    });
}

/*   event Liquidate(
        Id indexed id,
        address indexed caller,
        address indexed borrower,
        uint256 repaidAssets,
        uint256 repaidShares,
        uint256 seizedAssets,
        uint256 badDebtAssets,
        uint256 badDebtShares
    );
 */
pub fn update_liquidate(&self, log: &Log) {
    let market_id = FixedBytes::from(log.topics()[1]);
    let borrower = read_address(&log.topics()[3]);
    self.update(market_id, |m| {
        m.positions.retain(|p| p.address != borrower);
    });
}

/*   event AccrueInterest(Id indexed id, uint256 prevBorrowRate, uint256 interest, uint256 feeShares); */
pub fn update_accrue_interest(&self, log: &Log) {

    let market_id = FixedBytes::from(log.topics()[1]);
    let interest = read_u256(&log.data().data, 32);
    self.update(market_id, |m| {
        m.stats.total_borrow_assets += interest;
    });
}


/*   event SupplyCollateral(Id indexed id, address indexed caller, address indexed onBehalf, uint256 assets); */
pub fn update_supply_collateral(&self, log: &Log) {
    let market_id = FixedBytes::from(log.topics()[1]);
    let on_behalf = read_address(&log.topics()[3]);
    let assets = read_u256(&log.data().data, 0); // data: [caller (32), assets (32)]

    let found = self.update(market_id, |m| {
        if let Some(pos) = m.positions.iter_mut().find(|p| p.address == on_behalf) {
            pos.collateral_assets += assets;
            true
        } else {
            false
        }
    });

    if found.unwrap_or(false) {
        self.recompute_all_hf(market_id);
    } else {
        let pos = BorrowPosition {
            address: on_behalf,
            collateral_assets: assets,
            borrow_shares: U256::ZERO,
            market_id,
            cached_hf: None,
            onchain_checked: false,
            borrow_assets_usd: 0.0, // be careful
        };
        self.recompute_all_hf(market_id); // hf des autres positions à jour si besoin
        self.insert_pos(&pos);
    }
}

/*   event WithdrawCollateral(Id indexed id, address caller, address indexed onBehalf, address indexed receiver, uint256 assets); */
pub fn update_withdraw_collateral(&self, log: &Log) {
    let market_id = FixedBytes::from(log.topics()[1]);
    let on_behalf = read_address(&log.topics()[2]);
    let assets = read_u256(&log.data().data, 32); // data: [caller(0-32), assets(32-64)]

    self.update(market_id, |m| {
        if let Some(pos) = m.positions.iter_mut().find(|p| p.address == on_behalf) {
            pos.collateral_assets = pos.collateral_assets.saturating_sub(assets);
        }
    });

    self.recompute_all_hf(market_id);
}

}