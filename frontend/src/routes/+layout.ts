import { dev } from '$app/environment';

declare global {
    interface Window { twitchCallback: (state: string, code: string) => void }
}

export const prerender = !dev;
