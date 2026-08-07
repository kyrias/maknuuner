use std::time::Instant;

use topcoat::{
    Result,
    context::{Cx, app_context},
    router::{error::bad_request, page, query_params},
    runtime::{Event, shard},
    view::{Unescaped, View, component, view},
};
use tracing::instrument;

use crate::{
    lexicon::{Definition, Lemma, Lexicon, Phrase, Transcription, pos::PartOfSpeech},
    query::Query,
    string::SearchableString,
    web::title::Title,
};

#[query_params(error = bad_request)]
struct SearchQuery {
    query: Option<String>,
    raw: Option<bool>,
}

#[instrument(skip_all)]
#[page("/search")]
async fn search(cx: &Cx) -> Result {
    let params = query_params::<SearchQuery>(cx)?;

    let query_string = params.query.clone().unwrap_or_default();
    let raw = params.raw.unwrap_or(false);

    view! {
        signal query = query_string.clone();
        signal debounced = query_string;

        (Title::new("Search"))

        <script>
            (Unescaped::new_unchecked(
                r#"
                function debounce(func) {
                  let timer;
                  return (value) => {
                    clearTimeout(timer);
                    timer = setTimeout(() => { func(value); }, 150);
                  };
                }
                let debouncedSet;
            "#,
            ))
        </script>
        <form action="" method="get" rel="search" onsubmit="return false;">
            <input
                type="search"
                name="query"
                autocomplete="off"
                class="search"
                title="Search"
                placeholder="Search"
                :value=$(query.get())
                @input=$(|e: Event| {
                    let value = e.target.value;
                    query.set(value);
                    raw!(
                        r#"
                            if (!debouncedSet) {
                                debouncedSet = debounce(
                                    (value) => { ${debounced}.set(value) },
                                );
                            }
                            debouncedSet(${value});

                            let params = { query: ${value} };
                            if (${raw}.v) {
                                params.raw = "true";
                            }
                            window.history.replaceState({}, '', '?' + new URLSearchParams(params).toString());
                        "#
                    );
                })
            >

            if raw {
                <input type="hidden" name="raw" value="true">
            }
        </form>

        search_results(query: $(debounced.get()), raw: $(raw))
    }
}

#[instrument(skip_all, fields(query = query, raw = raw))]
#[shard]
async fn search_results(cx: &Cx, query: String, raw: bool) -> Result {
    let query = match Query::parse(&query) {
        Ok(query) => query,
        Err(error) => {
            tracing::error!(?error, "failed to parse query");
            return Err(bad_request("Could not parse query").into());
        }
    };

    let lexicon: &Lexicon = app_context(cx);

    let instant = Instant::now();
    let results = lexicon.search(&query, 100);
    let elapsed = instant.elapsed();

    let total_results = results.total_results();
    let returned_results = results.returned_results();
    let duration = format!("{} ms", elapsed.as_millis());
    tracing::info!(
        duration,
        total_results,
        returned_results,
        "Finished searching lexicon"
    );

    view! {
        <div class="results">
            for (rank, lemma) in results.into_iter() {
                result(lemma: lemma, rank: rank, raw: raw)
            }
        </div>
        <div class="result-count">
            "Displaying "
            (returned_results)
            " out of "
            (total_results)
            " results."
        </div>
        if raw {
            <div class="raw">
                "The following query took "
                (elapsed.as_secs_f64())
                "s to execute:"
                <pre>(format!("{query:#?}"))</pre>
            </div>
        }
    }
}

#[component]
async fn result(lemma: &Lemma, rank: f64, raw: bool) -> Result {
    // State used to only render the list of glosses if they differ from the previously rendered
    // definition.
    let mut previous_glosses = lemma
        .definitions
        .first()
        .map(|def| &def.glosses_english)
        .or_else(|| lemma.phrases.first().map(|ph| &ph.glosses_english))
        .unwrap();

    view! {
        <div id=(("lemma-", lemma.lowest_id)) class="result">
            <h3 lang="ar-PS">
                <span class="root" title="Root">
                    "("
                    (&lemma.root)
                    ")"
                </span>
                <span class="lemma" title="Lemma">(&lemma.lemma)</span>

                <a href=(("#lemma-", lemma.lowest_id))>
                    (Unescaped::new_unchecked("&sect;"))
                </a>
            </h3>

            <div class="inner">
                if !lemma.definitions.is_empty() {
                    <div class="definitions lemma-section">
                        <h4>"Definitions"</h4>
                        <ol>
                            for (idx, entry) in lemma.definitions.iter().enumerate() {
                                let glosses = if idx == 0
                                    || previous_glosses != &entry.glosses_english
                                {
                                    entry.glosses_english.as_slice()
                                } else {
                                    &[]
                                };
                                let _ = previous_glosses = &entry.glosses_english;
                                single_definition(definition: entry, glosses: glosses)
                            }
                        </ol>
                    </div>
                }

                if !lemma.phrases.is_empty() {
                    <div class="phrases lemma-section">
                        <h4>"Phrases"</h4>
                        <ol>
                            for phrase in &lemma.phrases {
                                single_phrase(phrase: phrase)
                            }
                        </ol>
                    </div>
                }

                let examples = lemma.example_usages().collect::<Vec<_>>();
                if !examples.is_empty() {
                    <div class="examples lemma-section" lang="ar-PS" dir="rtl">
                        <span class="example-header" title="Examples">
                            <span>"أمثلة"</span>
                            ": "
                        </span>
                        let num = examples.len();
                        for (idx, example) in examples.iter().enumerate() {
                            <span>(example)</span>
                            if (idx + 1) < num {
                                <span>" \u{2022} "</span>
                            }
                        }
                    </div>
                }

                if raw {
                    <div class="raw">
                        <p>
                            "Result has rank: "
                            (format!("{rank:0.5}"))
                        </p>
                        <pre>(format!("{lemma:#?}"))</pre>
                    </div>
                }
            </div>
        </div>
    }
}

#[component]
async fn single_definition(definition: &Definition, glosses: &[SearchableString]) -> Result {
    view! {
        <li id=(("def-", definition.id)) class="definition">
            <div class="form-header">
                <span lang="ar-PS">(&definition.form)</span>
                transcription(trans: &definition.transcription)
                part_of_speech(pos: definition.pos)
                glosses_msa(glosses: &definition.glosses_msa)
            </div>
            glosses_english(glosses: glosses)
        </li>
    }
}

#[component]
async fn single_phrase(phrase: &Phrase) -> Result {
    view! {
        <li id=(("ph-", phrase.id)) class="phrase">
            <div class="form-header">
                <span lang="ar-PS">(&phrase.form)</span>
                transcription(trans: &phrase.transcription)
                glosses_msa(glosses: &phrase.glosses_msa)
            </div>
            glosses_english(glosses: &phrase.glosses_english)
        </li>
    }
}

#[component]
async fn transcription(trans: &Transcription) -> Result {
    view! {
        <span class="transcription" title="IPA transcription">
            "("
            let num = trans.ipa.len();
            for (idx, ipa) in trans.ipa.iter().enumerate() {
                <span>
                    "/\u{2060}"
                    (ipa)
                    "\u{2060}/"
                </span>
                if (idx + 1) < num {
                    ", "
                }
            }
            ")"
        </span>
    }
}

#[component]
async fn part_of_speech(pos: Option<PartOfSpeech>) -> Result {
    let Some(pos) = pos else {
        return Ok(View::empty());
    };

    use crate::lexicon::pos::{
        noun::{Noun, NounFeature},
        verb::{Verb, VerbFeature},
    };

    fn noun_feature(nf: NounFeature) -> (&'static str, &'static str) {
        match nf {
            NounFeature::Singular => ("[sg.]", "Singular"),
            NounFeature::MasculineSingular => ("[m.sg.]", "Masculine singular"),
            NounFeature::FeminineSingular => ("[f.pl.]", "Feminine plural"),
            NounFeature::Dual => ("[d.]", "Dual"),
            NounFeature::Plural => ("[pl.]", "Plural"),
            NounFeature::MasculinePlural => ("[m.pl.]", "Masculine plural"),
            NounFeature::FemininePlural => ("[f.pl.]", "Feminine plural"),
        }
    }

    fn verb_feature(vf: VerbFeature) -> (&'static str, &'static str) {
        match vf {
            VerbFeature::Perfective => ("[p.]", "Perfective"),
            VerbFeature::Command => ("[c.]", "Command"),
            VerbFeature::Imperfective => ("[i.]", "Imperfective"),
        }
    }

    let (pos, pos_tooltip, feature) = match pos {
        PartOfSpeech::Noun(noun) => {
            let (kind, pos_tooltip, nf) = match noun {
                Noun::Plain(nf) => ("Noun", None, nf),
                Noun::Active(nf) => (
                    "Noun (act. part.)",
                    Some("Active participle deverbal noun"),
                    nf,
                ),
                Noun::Passive(nf) => (
                    "Noun (pass. part.)",
                    Some("Passive participle deverbal noun"),
                    nf,
                ),
                Noun::Proper(nf) => ("Noun (proper)", None, nf),
                Noun::Number(nf) => ("Noun (number)", None, nf),
                Noun::Quantifier(nf) => ("Noun (quantifier)", None, nf),
            };
            (kind, pos_tooltip, nf.map(noun_feature))
        }
        PartOfSpeech::Verb(verb) => match verb {
            Verb::Plain(vf) => ("Verb", None, Some(verb_feature(vf))),
            Verb::Nominal => ("Verb (nominal)", None, None),
            Verb::Pseudo => ("Verb (pseudo)", None, None),
        },
    };

    let feature = if let Some((feature, tooltip)) = feature {
        view! { <span title=(tooltip)>(feature)</span> }
    } else {
        view! {}
    };

    view! {
        <span class="pos">
            <span title=(pos_tooltip)>(pos)</span>
            " "
            (feature?)
        </span>
    }
}

#[component]
async fn glosses_msa(glosses: &[SearchableString]) -> Result {
    view! {
        if !glosses.is_empty() {
            <span class="msa-glosses">
                let num = glosses.len();
                <span>"("</span>
                <span title="Modern Standard Arabic">"msa. "</span>
                <span class="list">
                    for (idx, gloss) in glosses.iter().enumerate() {
                        <span>
                            if glosses.len() > 1 {
                                (idx + 1)
                                ". "
                            }
                            <span lang="ar-001" dir="rtl">(gloss)</span>
                            if (idx + 1) < num {
                                ", "
                            }
                        </span>
                    }
                </span>
                <span>")"</span>
            </span>
        }
    }
}

#[component]
async fn glosses_english(glosses: &[SearchableString]) -> Result {
    view! {
        if !glosses.is_empty() {
            <ol class="glosses">
                for gloss in glosses {
                    <li>(gloss)</li>
                }
            </ol>
        }
    }
}
