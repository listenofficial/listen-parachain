
#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::{prelude::*, result};

pub trait ListenHandler<RoomIndex, AccountId, DispatchErr> {
    fn get_room_council(room_id: RoomIndex) -> result::Result<Vec<AccountId>, DispatchErr>;
    fn get_prime(room_id: RoomIndex) -> result::Result<Option<AccountId>, DispatchErr>;
}

pub trait CollectiveHandler<RoomIndex, DispatchErr> {
    fn remove_room_collective_info(room_id: RoomIndex) -> result::Result<(), DispatchErr>;
}
