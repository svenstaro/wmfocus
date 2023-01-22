# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/)
and this project adheres to [Semantic Versioning](http://semver.org/).

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [1.4.0] - 2023-01-22
- Modernize all dependencies
- Fix border offset issue resulting from newer versions of i3

## [1.3.0] - 2021-10-22
- Highlight currently selected window (also adds `--textcolorcurrent`, `--textcolorcurrentalt`, `--bgcolorcurrent`) [#82](https://github.com/svenstaro/wmfocus/issues/82)

## [1.2.0] - 2021-07-11
- Add -e/--exit-keys to choose specific keys to quit wmfocus instead of every key (thanks @Rephobia)

## [1.1.5] - 2020-10-26
- Fix horizontal positions of labels for tabbed layout [#70](https://github.com/svenstaro/wmfocus/issues/70)

## [1.1.4] - 2020-08-07
- Rewrite argument parsing to be more robust [#63](https://github.com/svenstaro/wmfocus/issues/63)
- Add some debug information for font loading

## [1.1.3] - 2019-07-02
- Add offset parameter [#17](https://github.com/svenstaro/wmfocus/pull/17) (thanks @jeffmhubbard)

## [1.1.2] - 2019-01-08
- Bump some deps

## [1.1.1] - 2019-01-07
- Print X window client id instead of internal i3 id [#10](https://github.com/svenstaro/wmfocus/issues/10)

## [1.1.0] - 2018-12-14
- Make help colorful
- Properly handle tabbed and stacked windows [#5](https://github.com/svenstaro/wmfocus/issues/5)
- Make sure that labels don't overlap [#7](https://github.com/svenstaro/wmfocus/issues/7)
- Update to Rust 2018 edition

<!-- next-url -->
[Unreleased]: https://github.com/svenstaro/wmfocus/compare/v1.4.0...HEAD
[1.4.0]: https://github.com/svenstaro/wmfocus/compare/v1.3.0...v1.4.0
[1.3.0]: https://github.com/svenstaro/wmfocus/compare/v1.2.0...v1.3.0
[1.2.0]: https://github.com/svenstaro/wmfocus/compare/v1.1.5...v1.2.0
[1.1.5]: https://github.com/svenstaro/wmfocus/compare/v1.1.4...v1.1.5
[1.1.4]: https://github.com/svenstaro/wmfocus/compare/1.1.3...v1.1.4
[1.1.3]: https://github.com/svenstaro/wmfocus/compare/1.1.2...1.1.3
[1.1.2]: https://github.com/svenstaro/wmfocus/compare/1.1.1...1.1.2
[1.1.1]: https://github.com/svenstaro/wmfocus/compare/1.1.0...1.1.1
[1.1.0]: https://github.com/svenstaro/wmfocus/compare/1.0.2...1.1.0
