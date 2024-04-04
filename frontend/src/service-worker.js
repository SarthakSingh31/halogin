/// <reference lib="esnext" />
/// <reference lib="webworker" />

import { initializeApp } from "firebase/app";
import { getMessaging, onBackgroundMessage } from "firebase/messaging/sw";

const sw = /** @type {ServiceWorkerGlobalScope} */ (/** @type {unknown} */ (self));

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

const messaging = getMessaging(firebaseApp);
onBackgroundMessage(messaging, (payload) => {
    sw.clients.claim().then(() => { });
    for (let cname in sw.clients) {
        sw.clients.get(cname).then((client) => client?.postMessage({ "test": "message" }));
    }

    postMessage({ "test2": "message2" });

    console.log('[firebase-messaging-sw.js] Received background message ', payload);
    // Customize notification here
    const notificationTitle = 'Background Message Title';
    const notificationOptions = {
        body: 'Background Message body.',
        icon: '/firebase-logo.png'
    };

    sw.registration.showNotification(notificationTitle, notificationOptions);

    sw.reportError({ "test3": "message3" });
});