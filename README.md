# chrome-takeout-to-firefox

Small tool to import Google Chrome's Takeout History into Mozilla Firefox.

The primary location for this repository is on [Codeberg](https://codeberg.org/marie/chrome-takeout-to-firefox).

## Installation

### With Cargo:

```shell
cargo install --locked --git https://codeberg.org/marie/chrome-takeout-to-firefox --tag 0.1.0
```

## Usage
1. Go to [Google Takeout](https://takeout.google.com/settings/takeout) and export your Chrome history.

2. Extract the history json file. The name depends on your Google Accounts locale.

3. Lookup your Firefox profile path in `about:profiles`.

4. Close Firefox before starting the import.

5. To import your Chrome history into Firefox run the following command:

```
chrome-takeout-to-firefox ./path/to/your/history.json ~/path/to/your/firefox/profile/places.sqlite
```

## License
This project is licensed under MPL-2.0, because it uses code derived from the Firefox codebase.
