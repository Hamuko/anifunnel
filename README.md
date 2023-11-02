# anifunnel

Plex webhook service to automatically update your Anilist watching list.

## Description

anifunnel is a web server that will consume incoming Plex webhooks and update your Anilist watching list whenever you finish watching an episode.

The updating logic is rather conservative: the anime must be found within your watching list, and must have a matching episode count. So if you watch the first episode of a show, it will not add the show to your watching list on your own. Likewise, if you watch episode six of a show that matches a title where you have only the first two episodes marked as watched, it will also not update it, as it's looking for an anime that is at five episodes watched.

anifunnel implements fuzzy matching logic to allow the updating the work even if the titles aren't an exact match between your Plex library and Anilist. So for example "Boku no Hero Academia 6" can be matched against "Boku no Hero Academia (2022)" and "Uzaki-chan wa Asobitai! ω" can be matched against "Uzaki-chan wa Asobitai! Double".

It's also possible to customise the management logic on a per-anime basis for tricky edge cases using a management interface.

## Usage

### Authorization

Before starting to use anifunnel, you must fetch an API token.

In order to fetch your token, visit the following URL in your browser: https://anilist.co/api/v2/oauth/authorize?client_id=9878&response_type=token

Note that Anilist authorization tokens are valid for a year at a time.

### Running the server

To start the web server, use the following command:

```bash
anifunnel <ANILIST_TOKEN>
```

To get complete usage details, run `anifunnel --help`.

The alternative (and arguably easier) way to run anifunnel is to use the ready-made Docker image.

```bash
docker run \
    -p 8000:8000 \
    -e "ANILIST_TOKEN=xxx" \
    ghcr.io/hamuko/anifunnel:latest
```

Both `linux/amd64` and `linux/arm64` Docker image variants are available.

### Enabling webhooks in Plex

In order to send events from Plex to anifunnel, add the URL where your Plex server can reach anifunnel in Plex's Webhook settings.

The webhook handler responds on `/`, so if you were running the server on your local Plex server on port 8001, you'd use `http://127.0.0.1:8001/` as the webhook URL.

For more information, see https://support.plex.tv/articles/115002267687-webhooks/

Note that webhooks require a Plex Pass subscription.

### Multi-season shows

By default, anifunnel does not process episodes beyond the first season of a show. This is intentionally done as concatenating multiple different Anilist entries into a single Plex entry will reduce the likelihood that matching will succeed. If you want to enable multi-season matching anyways, you can use the `--multi-season` flag. Doing so will cause anifunnel to ignore Plex season numbers.

### Management interface

You can customise the anime title and episode number matching logic for anifunnel using the management interface. You can reach the management interface by going to `/admin` (or `/`, which will redirect to the correct URL) using your browser. anifunnel will load your watching list from Anilist and allow setting a custom title (exact match) and/or an episode offset.

**Important:** These overrides are currently stored in-memory only and will disappear once anifunnel is terminated. You will need to redo any applicable overrides after starting anifunnel up again.

## Disclaimer

This project is not associated or affiliated with Plex or Anilist in any way or form.
