<script lang="ts">
    import { onMount, onDestroy } from "svelte";

    let users = [
        { name: "basit", id: 1 },
        { name: "user2", id: 2 },
        { name: "user3", id: 3 },
    ];

    let currentChatPartner = users[0]; // Default chat partner

    let messages = [
        { text: "Hi there!", sender: users[0].id, timestamp: "10:00 AM" },
        { text: "Hello!", sender: users[0].id, timestamp: "10:01 AM" },
        { text: "How are you?", sender: users[1].id, timestamp: "10:02 AM" },
        {
            text: "I'm good, thank you!",
            sender: users[0].id,
            timestamp: "10:03 AM",
        },
    ];

    let newMessage = "";
    let ws: WebSocket | null = null;

    onMount(() => {
        ws = new WebSocket("ws://localhost:8080"); // Have to replace the  URL with actual server address.
        // currently getting 500 internal error..
        ws.onopen = () => {
            console.log("WebSocket connected");
        };

        ws.onmessage = (event) => {
            const message = JSON.parse(event.data);
            messages = [...messages, message];
        };
    });

    onDestroy(() => {
        ws?.close();
    });

    function sendMessage() {
        if (newMessage.trim() !== "") {
            const now = new Date();
            const hours =
                now.getHours() > 12
                    ? (now.getHours() - 12).toString()
                    : now.getHours() === 0
                      ? "12"
                      : now.getHours().toString();
            const minutes = now.getMinutes().toString().padStart(2, "0");
            const period = now.getHours() >= 12 ? "PM" : "AM";
            const time = `${hours}:${minutes} ${period}`;

            const message = {
                text: newMessage,
                sender: currentChatPartner.id,
                timestamp: time,
            };

            ws?.send(JSON.stringify(message));

            messages = [...messages, message];

            newMessage = "";
        }
    }

    function pinToBottom() {
        const chatContainer = document.querySelector(".chat-messages");
        // @ts-ignore
        chatContainer.scrollTop = chatContainer.scrollHeight;
    }
</script>

<div class="user-panel-container">
    <div class="user-panel">
        <h2>Chat Participants</h2>
        <ul class="user-list">
            <!-- svelte-ignore a11y-click-events-have-key-events -->
            <!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
            {#each users as user}
                <!-- svelte-ignore a11y-click-events-have-key-events -->
                <li on:click={() => (currentChatPartner = user)}>
                    <div class="user-profile"></div>
                    <span>{user.name}</span>
                </li>
            {/each}
        </ul>
    </div>

    <div class="chat-container">
        <div class="chat-header">
            <div class="user-profile"></div>
            <h1>Chatting with {currentChatPartner.name}</h1>
        </div>

        <div class="chat-messages">
            {#each messages as message}
                <div
                    class="chat-message {message.sender ===
                    currentChatPartner.id
                        ? 'my-message'
                        : 'other-message'}"
                >
                    <p>{message.text}</p>
                    <span class="timestamp">{message.timestamp}</span>
                </div>
            {/each}
        </div>

        <div class="input-container">
            <input
                type="text"
                bind:value={newMessage}
                placeholder="Type a message..."
            />
            <button on:click={sendMessage}>Send</button>
        </div>
    </div>
</div>

<style>
    .chat-container {
        width: 730px;
        height: 755px;
        /* margin: 50px auto; */
        background-color: #d7d2d2;
        /* border-radius: 10px;  */
        box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
        padding: 20px;
        box-sizing: border-box;
        display: flex;
        flex-direction: column;
        /* overflow: auto; */
    }

    .chat-messages {
        flex: 1;
        overflow-y: auto;
    }

    .chat-message {
        margin-bottom: 10px;
        padding: 10px;
        border-radius: 10px;
        max-width: 70%;
        word-wrap: break-word;
    }

    .my-message {
        background-color: #007bff;
        color: #fff;
        align-self: flex-end;
        border-top-right-radius: 0;
        margin-left: auto;
    }

    .other-message {
        background-color: #fff;
        color: #000;
        align-self: flex-start;
        border-top-left-radius: 0;
        margin-right: auto;
    }

    .input-container {
        display: flex;
        align-items: center;
        margin-top: 20px;
    }

    input[type="text"] {
        flex: 1;
        padding: 10px;
        border: 1px solid #ccc;
        border-radius: 4px;
        margin-right: 10px;
        word-wrap: break-word;
    }

    button {
        padding: 10px 20px;
        background-color: #007bff;
        color: #fff;
        border: none;
        border-radius: 4px;
        cursor: pointer;
    }

    button:hover {
        background-color: #0056b3;
    }
    .timestamp {
        font-size: 0.8em;
        color: #e9e0e0;
        margin-top: 5px;
    }

    .my-timestamp {
        text-align: right;
        color: white;
    }

    .other-timestamp {
        text-align: left;
    }
    @media only screen and (max-width: 768px) {
        .chat-container {
            width: 90%;
            margin: 20px auto;
            min-height: 500px;
        }
    }
    .user-panel-container {
        display: flex;
    }

    .user-panel {
        background-color: #f5f5f5;
        padding: 20px;
        border-right: 1px solid #ddd;
        width: 200px;
        max-height: calc(
            100% - 40px
        ); /* Subtracting padding from the total height */
        overflow-y: auto; /* Enable vertical scrolling */
        height: 756px;
        border: #918d8d solid thin;
    }

    .user-panel h2 {
        margin-bottom: 10px;
    }

    .user-list {
        list-style-type: none;
        padding: 0;
        margin: 0;
    }

    .user-list li {
        cursor: pointer;
        padding: 10px;
        border-bottom: 1px solid #ddd;
        display: flex;
        align-items: center;
    }

    .user-list li:hover {
        background-color: #eaeaea;
    }

    .user-profile {
        width: 40px;
        height: 40px;
        border-radius: 50%;
        background-color: #ccc;
        margin-right: 10px;
    }

    .chat-header {
        display: flex;
        align-items: center;
        padding: 10px;
        border-bottom: 1px solid #9f9b9b;
        /* background-color: #a9a2a2; */
        width: 100%;
        margin: 2px;
    }

    .chat-header h1 {
        margin-left: 20px;
        font-size: 18px;
    }
</style>
