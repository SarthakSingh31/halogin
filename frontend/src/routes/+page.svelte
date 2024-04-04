<script lang="ts">
    import { onMount } from "svelte";
    import Footer from "./Footer.svelte";
    import Home from "./Home.svelte";
    import NavBar from "./NavBar.svelte";

    import { initializeApp } from "firebase/app";
    import {
        getMessaging,
        getToken,
        onMessage,
        type Messaging,
    } from "firebase/messaging";

    const firebaseConfig = {
        apiKey: "AIzaSyANgj6nizbirKgzG6Zs2QlLM2Shq9VMNdU",
        authDomain: "halogin-1710687313349.firebaseapp.com",
        projectId: "halogin-1710687313349",
        storageBucket: "halogin-1710687313349.appspot.com",
        messagingSenderId: "751704262503",
        appId: "1:751704262503:web:ced964ca2c2d9b0a8091c2",
        measurementId: "G-QC4E2V19MY",
    };
    const firebaseApp = initializeApp(firebaseConfig);

    let messaging: Messaging | null = null;

    onMount(() => {
        // Initialize messaging with our Firebase configuration
        messaging = getMessaging(firebaseApp);

        onMessage(messaging, (payload) => {
            console.log("Foreground message: ", payload);
        });
    });

    function requestNotifPerm() {
        const vapidKey =
            "BPmHp-2qP776eWfXNtlcprGnBawas6V0MaCXN5TuSzyAuWFxQ3ybK9BPEKqB9hTFSD-4B8MyNX8fOTAGiugXKvQ";

        // Request permission for notifications
        Notification.requestPermission()
            .then((permission) => {
                if (permission === "granted") {
                    // Permission granted: Get the token
                    navigator.serviceWorker
                        .register("./service-worker.js")
                        .then((serviceWorkerRegistration) => {
                            navigator.serviceWorker.addEventListener(
                                "message",
                                console.log,
                            );

                            if (messaging !== null) {
                                getToken(messaging, {
                                    vapidKey,
                                    serviceWorkerRegistration,
                                })
                                    .then((fetchedToken) => {
                                        // Store the received token
                                        fetch(`test/${fetchedToken}`);
                                    })
                                    .catch((error) => {
                                        // Handle any errors in fetching the token
                                        console.error(
                                            "Error fetching token:",
                                            error,
                                        );
                                    });
                            } else {
                                console.error("Messaging was null");
                            }
                        });
                } else {
                    console.error("Permission not granted");
                }
            })
            .catch(console.log);
    }
</script>

<NavBar />
<button on:click={requestNotifPerm}>notif</button>
<Home />
<Footer />
