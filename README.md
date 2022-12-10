# plex-anihook

Plex webhook service to automatically update your Anilist watching list.

## Description

plex-anihook is a web server that will consume incoming Plex webhooks and update your Anilist watching list whenever you finish watching an episode.

The updating logic is rather conservative: the anime must be found within your watching list, and must have a matching episode count. So if you watch the first episode of a show, it will not add the show to your watching list on your own. Likewise, if you watch episode six of a show that matches a title where you have only the first two episodes marked as watched, it will also not update it, as it's looking for an anime that is at 

plex-anihook implements fuzzy matching logic to allow the updating the work even if the titles aren't an exact match between your Plex library and Anilist. So for example "Boku no Hero Academia 6" can be matched against "Boku no Hero Academia (2022)" and "Uzaki-chan wa Asobitai! ω" can be matched against "Uzaki-chan wa Asobitai! Double".

## Usage

### Authorization

Before starting to use plex-anihook, you must fetch an API token.

In order to fetch your token, visit the following URL in your browser: https://anilist.co/api/v2/oauth/authorize?client_id=9878&response_type=token

Note that Anilist authorization tokens are valid for a year at a time.

### Running the server

To start the web server, use the following command:

```bash
plex-anihook <ANILIST_TOKEN>
```

To get complete usage details, run `plex-anihook --help`.

### Enabling webhooks in Plex

In order to send events from Plex to plex-anihook, add the URL where your Plex server can reach plex-anihook in Plex's Webhook settings.

The webhook handler responds on `/`, so if you were running the server on your local Plex server on port 8001, you'd use `http://127.0.0.1:8001/` as the webhook URL.

For more information, see https://support.plex.tv/articles/115002267687-webhooks/

Note that webhooks require a Plex Pass subscription.
