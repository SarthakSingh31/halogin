<script lang="ts">
    import { Avatar, Button, Card } from "flowbite-svelte";

    export let data: {
        givenName: string;
        familyName: string;
        pronouns: string;
        pfpImageUrl: string;
        profileDesc: string;
        contentDesc: string;
        audienceDesc: string;
    };
    export let defaults: {
        givenName: string;
        familyName: string;
        pronouns: string;
        profileDesc: string;
        contentDesc: string;
        audienceDesc: string;
    };
    let defaultClass: string;
    export { defaultClass as class };

    export let youtubeChannels: YoutubeChannel[];
    export let twitchAccounts: TwitchAccount[];

    $: givenName = data.givenName == "" ? defaults.givenName : data.givenName;
    $: familyName =
        data.familyName == "" ? defaults.familyName : data.familyName;
    $: pronouns = data.pronouns == "" ? defaults.pronouns : data.pronouns;
    $: profileDesc =
        data.profileDesc == "" ? defaults.profileDesc : data.profileDesc;
    $: contentDesc =
        data.contentDesc == "" ? defaults.contentDesc : data.contentDesc;
    $: audienceDesc =
        data.audienceDesc == "" ? defaults.audienceDesc : data.audienceDesc;
</script>

<div class={defaultClass}>
    <Card size="xl">
        <div class="flex mb-2">
            <Avatar size="lg" src={data.pfpImageUrl} border />
            <div class="ml-4">
                <h5
                    class="text-4xl font-bold tracking-tight text-gray-900 dark:text-white"
                >
                    {givenName}
                    {familyName}
                </h5>
                <span class="text-2xl tracking-tight">
                    {pronouns}
                </span>
            </div>
        </div>
        <div class="text-gray-900 dark:text-white">
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">Who am I?</h6>
                {profileDesc}
            </div>
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">
                    What do I make?
                </h6>
                {contentDesc}
            </div>
            <div>
                <h6 class="mt-1 text-md font-bold tracking-tight">
                    Who watches my content?
                </h6>
                {audienceDesc}
            </div>
        </div>
        <div>
            {#each youtubeChannels as channel}
                <Button
                    on:click={() => {
                        window.open(
                            `https://www.youtube.com/${channel.snippet.customUrl}`,
                        );
                    }}
                >
                    <Avatar
                        size="xs"
                        src={channel.snippet.thumbnails.high.url}
                        border
                    />
                    <span class="ml-2">{channel.snippet.title}</span>
                </Button>
                <span class="ml-2">
                    Subscriber Count: {channel.statistics.subscriberCount}
                </span>
                <span class="ml-2">
                    Viewer per video: {(
                        channel.statistics.viewCount /
                        channel.statistics.videoCount
                    ).toFixed(2)}
                </span>
            {/each}
        </div>
        <div>
            {#each twitchAccounts as account}
                <Button
                    on:click={() => {
                        window.open(
                            `https://www.twitch.tv/${account.display_name}`,
                        );
                    }}
                >
                    <Avatar size="xs" src={account.profile_image_url} border />
                    <span class="ml-2">{account.display_name}</span>
                </Button>
                <span class="ml-2">
                    Follower Count: {account.follower_count}
                </span>
                <span class="ml-2">
                    Subscriber Count: {account.subscriber_count}
                </span>
            {/each}
        </div>
    </Card>
</div>
