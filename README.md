# Maknuuner

Maknuuner is a web UI that tries to provide an easy-to-use interface for the [Maknuune](http://www.palestine-lexicon.org/) lexicon of Palestinian Arabic.
This came to be as the PDF is not very easy to search through, and the Google Data Studio interface is frankly terrible and regularly just stops working.


## Running it

Running Maknuuner requires access to the Maknuune TSV file and the Amiri font.
There is an xtask for fetching and unpacking the required files which can be executed using `cargo xtask assets`.
After that has been done the server can either be run through `cargo run` or using `topcoat dev` to get automatic rebuilds and reloads when modifying the code.

By default the server listens on `127.0.0.1:3000` but that can be overridden using the `HOST` and `PORT` environment variables.

(TODO: Integrate the entire `topcoat` CLI as an xtask to get access to it without requiring installing a separate tool?)


## Cargo xtasks

<dl>
    <dt>`cargo xtask assets`</dt>
    <dd>Downloads and unpacks the required assets.  (Maknuune dataset and the Amiri font.)</dd>
    <dt>`cargo xtask build`</dt>
    <dd>Convenience xtask for preparing the assets and building the server.</dd>
    <dt>`cargo xtask dist`</dt>
    <dd>Prepares the assets, builds the server, and copies the required files to run the server to the `dist/` directory.</dd>
</dl>
