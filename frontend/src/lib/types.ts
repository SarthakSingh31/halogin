type YoutubeChannel = {
    id: string;
    snippet: {
        title: string;
        customUrl: string;
        thumbnails: {
            high: {
                url: string;
            };
        };
    };
    statistics: {
        viewCount: number;
        videoCount: number;
        subscriberCount: number;
    };
};

type TwitchAccount = {
    display_name: string;
    follower_count: number;
    subscriber_count: number;
    profile_image_url: string;
};