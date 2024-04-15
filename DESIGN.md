## Chat Interface

```rust
struct NewMessage {
    content: String,
    contract_change: Option<MessageContractChange>,
    attachment: Option<NewMessageFile>,
    change_selected_campaign_to: Option<Uuid>,
}
enum MessageContractChange {
    ProposedByCompany {
        campaign_id: Uuid,
        payout: Cents,
    },
    AcceptedByCreator,
    WithdrawnByCompany,
    CancelledByCreator,
    FinishedByCreator,
    ApprovedByCompany,
}
struct NewMessageFile {
    name: String,
    // This comes in base64 encoded
    content: Box<[u8]>,
    content_type: String,
}
struct Message<T> {
    id: usize,
    content: String,
    contract_change: Option<MessageContractChange>,
    attachment: Option<MessageFile>,
    // TODO: Maybe this needs to be expanded to a full campaign struct
    change_selected_campaign_to: Option<Uuid>,
    timestamp: OffsetDateTime,
    from_user: T,
}
struct MessageFile {
    name: String,
    link: String,
}
enum ActivityState {
    Online,
    Away,
    Offline,
}
struct UserInfo {
    id: Uuid,
    given_name: String,
    familiy_name: String,
    logo_url: String,
    status: ActivityState,
    company: Option<Company>
}
struct Company {
    id: Uuid,
    name: String,
    logo_url: String,
}
```

### Function Calls:

- Create Chat Room
Send:
```rust
struct CreateChatRoom {
    message: NewMessage,
    direction: CreateChatRoomDirection,
}
// Get the user_id from the websocket session
enum CreateChatRoomDirection {
    UserToCompany {
        company_id: Uuid,
    },
    CompanyToUser {
        company_id: Uuid,
        to_user_id: Uuid,
    }
}
```
Recv:
```rust
struct CreateChatRoomResp {
    room_id: Uuid,
}
```
- Send a message
Send:
```rust
struct SendMessage {
    room_id: Uuid,
    from_user_id: Uuid,
    message: NewMessage,
}
```
Recv:
```rust
struct SendMessageResp {
    message_id: Uuid,
}
```
- Status update of each page
Send:
```rust
struct CurrentlyViewingUpdate {
    viewing: bool
}
```
- Typing update
Send:
```rust
struct CurrentlyTypingUpdate {
    typing: bool
}
```
- List Rooms
Send:
```rust
struct ListRooms {}
```
Recv:
```rust
struct Rooms {
    rooms: Vec<Room>,
}
struct Room {
    id: Uuid,
    // Sorted by most active user except the current user
    users: Vec<UserInfo>,
    last_message: Message,
    new_messages: usize,
}
```
- Query Room
Send:
```rust
struct RoomQuery {
    room_id: Uuid,
    message_qty: Option<usize>,
    message_before: Option<usize>,
}
```
Recv:
```rust
struct DeatiledRoom {
    id: Uuid,
    users: Vec<UserInfo>,
    messages: Vec<Message>,
    new_messages: usize,
    // TODO: Maybe this needs to be expanded to a full campaign struct
    selected_campaign_id: Option<Uuid>,
}
```
- Update Last Seen Message
Send:
```rust
struct UpdateLastSeen {
    room_id: Uuid,
    seen_till: usize,
}
```

### Notifications:

- New Room:
```rust
struct RoomCreatedWithYou {
    room_id: Uuid,
    message: Message<UserInfo>,
    // TODO: Maybe this needs to be expanded to a full campaign struct
    selected_campaign_id: Option<Uuid>,
}
```
- New Message:
```rust
struct NewMessage {
    room_id: Uuid,
    message: Message<UserInfo>,
}
```
- User Activity Change
```rust
struct ActivityChange {
    user_id: Uuid,
    activity_state: ActivityState,
}
```
- User Typing Change
```rust
struct TypingChange {
    user_id: Uuid,
    currently_typing: bool,
}
```
- User Last View Change
```rust
struct NewLastView {
    user_id: Uuid,
    room_id: Uuid,
    message_id: usize,
}
```