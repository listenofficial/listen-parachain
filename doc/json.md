```
{
  "CurrencyIdOf":"CurrencyId",
  "TokenSymbol":{
       "_enum":["LT", "DOT", "ACA", "BTC", "KSM", "PHA", "PLM"]
   },
  "EvmAddress":"Hash",
  "CurrencyId": {
    "_enum": {
        "Token":"TokenSymbol",
        "DEXShare":"(TokenSymbol, TokenSymbol)",
        "ERC20":"EvmAddress"
    }
  },
  "NativeBalanceOf":"Balance",
  "MultiBalanceOf":"Balance",
  "Amount": "Balance",
  "AllProps": {
    "picture": "u32",
    "text": "u32",
    "video": "u32"
  },

  "CreateCost": {
    "Ten": "Balance",
    "Hundred": "Balance",
    "FiveHundred": "Balance",
    "TenThousand": "Balance",
    "NoLimit": "Balance"
  },

  "AssetChangeableParams": {
    "_enum": {
      "MintPledge": "Balance",
      "BurnPledge": "Balance",
      "MintMinAmount": "Balance",
      "BurnMinAmount": "Balance",
      "MintExistsHowLong": "AssetTime",
      "MintPeriod": "AssetTime",
      "BurnExistsHowLong": "AssetTime",
      "MaxLenOfMint": "u32",
      "MaxLenOfBurn": "u32"
    }
  },
  "MintVote": {
    "start_block": "BlockNumber",
    "pass_block": "Option<BlockNumber>",
    "mint_block": "Option<BlockNumber>",
    "mint_man": "AccountId",
    "asset_id": "AssetId",
    "amount": "Balance",
    "approve_list": "Vec<AccountId>",
    "reject_list": "Vec<AccountId>",
    "technical_reject": "Option<AccountId>"
  },
  "AssetVote": {
    "_enum": [
      "Approve",
      "Reject"
    ]
  },
  "BurnInfo": {
    "start_block": "BlockNumber",
    "burn_man": "AccountId",
    "asset_id": "AssetId",
    "amount": "Balance",
    "foundation_tag_man": "Option<AccountId>"
  },
  "RoomRewardInfo": {
    "total_person": "u32",
    "already_get_count": "u32",
    "total_reward": "Balance",
    "already_get_reward": "Balance",
    "per_man_reward": "Balance"
  },
  "RedPacket": {
    "id": "u128",
    "currency_id": "CurrencyId",
    "boss": "AccountId",
    "total": "Balance",
    "lucky_man_number": "u32",
    "already_get_man": "BTreeSet<AccountId>",
    "min_amount_of_per_man": "Balance",
    "already_get_amount": "Balance",
    "end_time": "BlockNumber"
  },
  "Audio": {
    "ten_seconds": "u32",
    "thirty_seconds": "u32",
    "minutes": "u32"
  },
  "DisbandVote": {
    "approve_man": "BTreeSet<AccountId>",
    "reject_man": "BTreeSet<AccountId>",
    "approve_total_amount":"Balance",
    "reject_total_amount":"Balance"
  },
  "GroupMaxMembers": {
    "_enum": [
      "Ten",
      "Hundred",
      "FiveHundred",
      "TenThousand",
      "NoLimit"
    ]
  },
  "VoteType": {
    "_enum": [
      "Approve",
      "Reject"
    ]
  },
  "RewardStatus": {
    "_enum": [
      "Get",
      "NotGet",
      "Expire"
    ]
  },
  "GroupInfo": {
    "group_id": "u64",
    "create_payment": "Balance",
    "last_block_of_get_the_reward": "BlockNumber",
    "pledge_amount":"Balance",
    "group_manager": "AccountId",
    "max_members": "GroupMaxMembers",
    "group_type": "Vec<u8>",
    "join_cost": "Balance",
    "props": "AllProps",
    "audio": "Audio",
    "total_balances": "Balance",
    "group_manager_balances": "Balance",
    "now_members_number": "u32",
    "last_remove_height": "BlockNumber",
    "last_disband_end_hight": "BlockNumber",
    "disband_vote": "DisbandVote",
    "this_disband_start_time": "BlockNumber",
    "is_voting": "bool",
    "create_time": "Moment",
    "create_block": "BlockNumber",
    "consume": "BTreeMap<AccountId, Balance>",
    "council": "Vec<(AccountId, Balance)>",
    "black_list":"Vec<AccountId>"
  },
  "InvitePaymentType": {
    "_enum": [
      "inviter_pay",
      "invitee_pay"
    ]
  },
  "ListenerType": {
    "_enum": [
      "group_manager",
      "common",
      "honored_guest"
    ]
  },
  "PersonInfo": {
    "props": "AllProps",
    "audio": "Audio",
    "cost": "Balance",
    "rooms": "Vec<(u64, RewardStatus)>"
  },
  "AudioPrice": {
    "ten_seconds": "Balance",
    "thirty_seconds": "Balance",
    "minutes": "Balance"
  },
  "PropsPrice": {
    "picture": "Balance",
    "text": "Balance",
    "video": "Balance"
  },
  "DisbandTime": {
    "Ten": "BlockNumber",
    "Hundred": "BlockNumber",
    "FiveHundred": "BlockNumber",
    "TenThousand": "BlockNumber",
    "NoLimit": "BlockNumber"
  },
  "RemoveTime": {
    "Ten": "BlockNumber",
    "Hundred": "BlockNumber",
    "FiveHundred": "BlockNumber",
    "TenThousand": "BlockNumber",
    "NoLimit": "BlockNumber"
  },

  "VestingInfo": {
    "locked": "Balance",
    "unlock_amount_of_per_duration": "Balance",
    "block_number_of_per_duration": "BlockNumber",
    "starting_block": "BlockNumber"
    },

  "XCurrencyId": {
    "chain_id":"ChainId",
    "currency_id": "Vec<u8>"
    },

    "ChainId": {
      "_enum": {
        "RelayChain": null,
        "ParaChain": "ParaId"
        }
    },
    
    "ParaId":"u32",

  "LookupSource": "MultiAddress",
  "Address": "MultiAddress"

}
```
