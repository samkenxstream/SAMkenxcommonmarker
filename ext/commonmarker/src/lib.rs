extern crate core;

use std::path::PathBuf;

use ::syntect::highlighting::ThemeSet;
use comrak::{
    adapters::SyntaxHighlighterAdapter,
    markdown_to_html, markdown_to_html_with_plugins,
    plugins::syntect::{SyntectAdapter, SyntectAdapterBuilder},
    ComrakOptions, ComrakPlugins,
};
use magnus::{
    define_module, exception, function, r_hash::ForEach, scan_args, Error, RHash, Symbol, Value,
};

mod options;
use options::iterate_options_hash;

mod plugins;
use plugins::{
    syntax_highlighting::{
        fetch_syntax_highlighter_path, fetch_syntax_highlighter_theme,
        SYNTAX_HIGHLIGHTER_PLUGIN_DEFAULT_THEME,
    },
    SYNTAX_HIGHLIGHTER_PLUGIN,
};

mod utils;

pub const EMPTY_STR: &str = "";

fn commonmark_to_html<'a>(args: &[Value]) -> Result<String, magnus::Error> {
    let args = scan_args::scan_args(args)?;
    let (rb_commonmark,): (String,) = args.required;
    let _: () = args.optional;
    let _: () = args.splat;
    let _: () = args.trailing;
    let _: () = args.block;

    let kwargs = scan_args::get_kwargs::<_, (), (Option<RHash>, Option<RHash>), ()>(
        args.keywords,
        &[],
        &["options", "plugins"],
    )?;
    let (rb_options, rb_plugins) = kwargs.optional;

    let mut comrak_options = ComrakOptions::default();

    if let Some(rb_options) = rb_options {
        rb_options.foreach(|key: Symbol, value: RHash| {
            iterate_options_hash(&mut comrak_options, key, value)?;
            Ok(ForEach::Continue)
        })?;
    }

    if let Some(rb_plugins) = rb_plugins {
        let mut comrak_plugins = ComrakPlugins::default();

        let syntax_highlighter: Option<&dyn SyntaxHighlighterAdapter>;
        let adapter: SyntectAdapter;

        let theme = match rb_plugins.get(Symbol::new(SYNTAX_HIGHLIGHTER_PLUGIN)) {
            Some(syntax_highlighter_options) => {
                fetch_syntax_highlighter_theme(syntax_highlighter_options)?
            }
            None => SYNTAX_HIGHLIGHTER_PLUGIN_DEFAULT_THEME.to_string(), // no `syntax_highlighter:` defined
        };

        let path = match rb_plugins.get(Symbol::new(SYNTAX_HIGHLIGHTER_PLUGIN)) {
            Some(syntax_highlighter_options) => {
                fetch_syntax_highlighter_path(syntax_highlighter_options)?
            }
            None => PathBuf::from("".to_string()), // no `syntax_highlighter:` defined
        };

        if !path.eq(&PathBuf::from("".to_string())) && !path.exists() {
            return Err(Error::new(
                exception::arg_error(),
                format!("path does not exist"),
            ));
        }

        if theme.is_empty() && path.exists() {
            return Err(Error::new(
                exception::arg_error(),
                "`path` also needs `theme` passed into the `syntax_highlighter`",
            ));
        }
        if path.exists() && !path.is_dir() {
            return Err(Error::new(
                exception::arg_error(),
                "`path` needs to be a directory",
            ));
        }

        if path.exists() {
            let builder = SyntectAdapterBuilder::new();
            let mut ts = ThemeSet::load_defaults();

            match ts.add_from_folder(&path) {
                Ok(_) => {}
                Err(e) => {
                    return Err(Error::new(
                        exception::arg_error(),
                        format!("failed to load theme set from path: {e}"),
                    ));
                }
            }

            ts.themes.get(&theme).ok_or_else(|| {
                Error::new(
                    exception::arg_error(),
                    format!("theme `{}` does not exist", theme),
                )
            })?;

            adapter = builder.theme_set(ts).theme(&theme).build();

            syntax_highlighter = Some(&adapter);
        } else if theme.is_empty() || theme == "none" {
            syntax_highlighter = None;
        } else {
            ThemeSet::load_defaults()
                .themes
                .get(&theme)
                .ok_or_else(|| {
                    Error::new(
                        exception::arg_error(),
                        format!("theme `{}` does not exist", theme),
                    )
                })?;
            adapter = SyntectAdapter::new(&theme);
            syntax_highlighter = Some(&adapter);
        }

        comrak_plugins.render.codefence_syntax_highlighter = syntax_highlighter;

        Ok(markdown_to_html_with_plugins(
            &rb_commonmark,
            &comrak_options,
            &comrak_plugins,
        ))
    } else {
        Ok(markdown_to_html(&rb_commonmark, &comrak_options))
    }
}

#[magnus::init]
fn init() -> Result<(), Error> {
    let module = define_module("Commonmarker")?;

    module.define_module_function("commonmark_to_html", function!(commonmark_to_html, -1))?;

    Ok(())
}
