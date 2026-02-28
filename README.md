![anifunnel](https://burakku.com/projects/anifunnel/banner.png)

Plex webhook service to automatically update your Anilist watching list.

## Description

anifunnel is a web server that will consume incoming Plex webhooks and update your Anilist watching list whenever you finish watching an episode. Due to this design, anifunnel only needs to authenticate against Anilist and doesn't require access to your Plex instance. It's also written in Rust to aim for relatively low system resource usage.

The updating logic is rather conservative: the anime must be found within your watching (or rewatching) list, and must have a matching episode count. So if you watch the first episode of a show, it will not add the show to your watching list on your own. Likewise, if you watch episode six of a show that matches a title where you have only the first two episodes marked as watched, it will also not update it, as it's looking for an anime that is at five episodes watched.

anifunnel implements fuzzy matching logic to allow the updating the work even if the titles aren't an exact match between your Plex library and Anilist. So for example "Boku no Hero Academia 6" can be matched against "Boku no Hero Academia (2022)" and "Uzaki-chan wa Asobitai! ω" can be matched against "Uzaki-chan wa Asobitai! Double".

It's also possible to customise the matching logic on a per-anime basis for tricky edge cases using a management interface.

## Usage

### Running the server

To start the web server, simply run the anifunnel binary:

```bash
anifunnel
```

To get complete usage details, run `anifunnel --help`.

The alternative (and arguably easier) way to run anifunnel is to use the ready-made Docker image.

```bash
docker run \
    -p 8000:8000 \
    -v /path/to/anifunnel/db/directory:/db \
    ghcr.io/hamuko/anifunnel:latest
```

Example Docker Compose manifest:

```yaml
version: '3.7'
services:
  anifunnel:
    container_name: anifunnel
    image: ghcr.io/hamuko/anifunnel:latest
    ports:
      - 8000:8000
    volumes:
      - /path/to/anifunnel/db/directory:/db
    restart: on-failure
```

Both `linux/amd64` and `linux/arm64` Docker image variants are available.

### Authorization

After you've started the anifunnel server, open anifunnel's URL (e.g. http://localhost:8000/) in your browser. This will open the management interface and will prompt you to authenticate with Anilist. This is required for the updates to work. Once you have successfully authenticated, you will see a list of anime on your Anilist watching list and the duration how long your token will be valid for.

### Enabling webhooks in Plex

In order to send events from Plex to anifunnel, add the URL where your Plex server can reach anifunnel in Plex's Webhook settings.

The webhook handler responds on `/`, so if you were running the server on your local Plex server on port 8001, you'd use `http://127.0.0.1:8001/` as the webhook URL.

For more information, see https://support.plex.tv/articles/115002267687-webhooks/

Note that webhooks require a Plex Pass subscription.

### Adjusting matching

You can customise the anime title and episode number matching logic for anifunnel using the same management interface as is used for authorizing. anifunnel will load your watching list from Anilist and allow setting a custom title (exact match) and/or an episode offset.

### Username filtering

anifunnel processes events for all Plex users by default. If you are using a multi-user Plex instance, you can limit processing of webhook events to a single user with the `--plex-user` argument / `ANILIST_PLEX_USER` environment variable.

## Disclaimer

This project is not associated or affiliated with Plex or Anilist in any way or form.
