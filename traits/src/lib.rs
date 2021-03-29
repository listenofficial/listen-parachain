use sp_std::{prelude::*, result};

pub trait ListenHandler<RoomIndex, RoomInfo, DispatchErr> {
    fn get_room_info(room_id: RoomIndex) -> result::Result<RoomInfo, DispatchErr>;
}
