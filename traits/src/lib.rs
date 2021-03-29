use sp_std::{prelude::*, result};

pub trait ListenHandler<RoomIndex, AccountId, DispatchErr> {
    fn get_room_council(room_id: RoomIndex) -> result::Result<Vec<AccountId>, DispatchErr>;
}
