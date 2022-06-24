use super::*;

pub struct SystemBaseCallFilter;
impl Contains<Call> for SystemBaseCallFilter {
	fn contains(call: &Call) -> bool {
		!matches!(
			call,
			Call::Balances(_) |
				Call::Room(pallet_room::Call::ask_for_disband_room { .. }) |
				Call::Room(pallet_room::Call::vote { .. }) |
				Call::Room(pallet_room::Call::pay_out { .. }) |
				Call::Room(pallet_room::Call::disband_room { .. }) |
				Call::RoomTreasury(_) |
				Call::Nft(_)
		)
	}
}

pub struct DaoBaseCallFilter;
impl Contains<Call> for DaoBaseCallFilter {
	fn contains(call: &Call) -> bool {
		match call {
			Call::Room(func) => match func {
				pallet_room::Call::manager_get_reward { .. } |
				pallet_room::Call::update_join_cost { .. } |
				pallet_room::Call::set_room_privacy { .. } |
				pallet_room::Call::set_max_number_for_room_members { .. } |
				pallet_room::Call::remove_someone_from_blacklist { .. } |
				pallet_room::Call::set_council_members { .. } |
				pallet_room::Call::add_council_member { .. } |
				pallet_room::Call::remove_council_member { .. } |
				pallet_room::Call::remove_someone { .. } => true,
				_ => false,
			},
			_ => false,
		}
	}
}
