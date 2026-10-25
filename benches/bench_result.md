# Benchmark Results

**10/25/2026 (Baseline before optimization)**

| Benchmark Name                         | Result                |
|----------------------------------------|----------------------:|
| `bench_tokenize_small`                 | `595.97 ns`           |
| `bench_parse_small`                    | `1.6041 µs` (1604 ns) |
| `bench_tokenize_realistic`             | `2.0815 µs` (2082 ns) |
| `bench_parse_realistic_new_parser`     | `7.1128 µs` (7113 ns) |
| `bench_parse_realistic_reused_parser`  | `6.9700 µs` (6970 ns) |
| `bench_parse_schema_defs`              | `10.431 µs`           |
| `bench_parse_batch_20`                 | `145.29 µs`           |
| `bench_stream_parser_chunked_32b`      | `33.388 µs`           |
| `bench_expr_to_string`                 | `2.8853 µs` (2885 ns) |

**10/25/2026 - Step 1: Cursor-based parsing, zero happy-path token cloning, and in-place `Atom` string moves**
> Replaced `recording` / `recorded` / `scanned` speculative token cloning with index-cursor traversal (`pos: usize`), eliminated `Expr` AST deep-cloning in `Parser::parse`, moved owned `String` tokens directly into `Atom` (`Symbol`, `Keyword`, `String`) and `Expr::Comment` without re-allocating, skipped `parse::<i64>()` on non-numeric symbols, and reused `VecDeque` capacity in `Parser::clear_tokens` / `Parser::clear_exprs`.

| Benchmark Name                         | Result                | vs. Baseline |
|----------------------------------------|----------------------:|-------------:|
| `bench_tokenize_small`                 | `606.87 ns`           |            — |
| `bench_parse_small`                    | `745.38 ns`           |   **-53.5%** |
| `bench_tokenize_realistic`             | `2.0796 µs` (2080 ns) |            — |
| `bench_parse_realistic_new_parser`     | `2.7454 µs` (2745 ns) |   **-61.4%** |
| `bench_parse_realistic_reused_parser`  | `2.6922 µs` (2692 ns) |   **-61.4%** |
| `bench_parse_schema_defs`              | `4.1881 µs`           |   **-59.8%** |
| `bench_parse_batch_20`                 | `55.866 µs`           |   **-61.5%** |
| `bench_stream_parser_chunked_32b`      | `13.216 µs`           |   **-60.4%** |
| `bench_expr_to_string`                 | `2.8984 µs` (2898 ns) |            — |

**10/25/2026 - Step 2: Single-buffer `Expr::write_to` & fast stack integer formatting**
> Replaced recursive `Expr::to_string` intermediate `Vec<String>` + `.join(" ")` allocations with a single pre-allocated `String` (`Expr::estimated_len`) and direct `write_to(&self, buf: &mut String)` traversal, plus stack-buffer integer formatting (`push_u64`, `push_i64`).

| Benchmark Name                         | Result                | vs. Baseline |
|----------------------------------------|----------------------:|-------------:|
| `bench_expr_to_string`                 | `345.56 ns`           |   **-88.0%** |

**10/25/2026 - Step 3: Fast lookup-table tokenizer, direct `tokens` push, and `string_pool` recycling**
> Replaced the 4096-byte stack read loop and intermediate `Vec<String>` in `Parser::tokenize` with `read_to_end(&mut self.byte_cache)`, a 256-entry `IS_DELIM` lookup table, fast inner scans for symbols and consecutive spaces, and an opt-in `string_pool` activated on `Parser::clear` / `Parser::clear_tokens` to recycle consumed `String` allocations across reused parser runs.

| Benchmark Name                         | Result                | vs. Baseline |
|----------------------------------------|----------------------:|-------------:|
| `bench_tokenize_small`                 | `449.73 ns`           |   **-24.5%** |
| `bench_parse_small`                    | `566.11 ns`           |   **-64.7%** |
| `bench_tokenize_realistic`             | `1.7496 µs` (1750 ns) |   **-15.9%** |
| `bench_parse_realistic_new_parser`     | `2.3831 µs` (2383 ns) |   **-66.5%** |
| `bench_parse_realistic_reused_parser`  | `1.8330 µs` (1833 ns) |   **-73.7%** |
| `bench_parse_schema_defs`              | `3.7945 µs`           |   **-63.6%** |
| `bench_parse_batch_20`                 | `50.084 µs`           |   **-65.5%** |

**10/25/2026 - Step 4: Direct byte-slice `SliceParser` (`Parser::parse_str`) & `StreamParser` fast path**
> Added `SliceParser<'a>` and `Parser::parse_str` to parse S-expressions directly from `&str` / `&[u8]` without allocating an intermediate `VecDeque<String>` token queue or heap `String`s for delimiters, spaces, and numbers. Updated `StreamParser` to buffer incoming chunks in `byte_cache`, check balanced parenthesis readiness before attempting to parse, and parse complete expressions directly via `SliceParser`.

| Benchmark Name                         | Result                | vs. Baseline           | Speedup   |
|----------------------------------------|----------------------:|-----------------------:|----------:|
| `bench_tokenize_small`                 | `449.73 ns`           |             **-24.5%** |  **1.3x** |
| `bench_parse_small`                    | `566.11 ns`           |             **-64.7%** |  **2.8x** |
| `bench_parse_str_small` *(direct)*     | `172.64 ns`           | *-89.2% vs. baseline*  |  **9.3x** |
| `bench_tokenize_realistic`             | `1.7496 µs` (1750 ns) |             **-15.9%** |  **1.2x** |
| `bench_parse_realistic_new_parser`     | `2.3831 µs` (2383 ns) |             **-66.5%** |  **3.0x** |
| `bench_parse_realistic_reused_parser`  | `1.8330 µs` (1833 ns) |             **-73.7%** |  **3.8x** |
| `bench_parse_str_realistic` *(direct)* | `896.46 ns`           | *-87.4% vs. baseline*  |  **7.9x** |
| `bench_parse_schema_defs`              | `3.7945 µs`           |             **-63.6%** |  **2.7x** |
| `bench_parse_batch_20`                 | `50.084 µs`           |             **-65.5%** |  **2.9x** |
| `bench_stream_parser_chunked_32b`      | `1.8677 µs` (1868 ns) |             **-94.4%** | **17.9x** |
| `bench_expr_to_string`                 | `346.06 ns`           |             **-88.0%** |  **8.3x** |

**10/25/2026 - Step 5: Zero-Copy AST (`TypeValue<'a>` & `Expr<'a>`) & Contiguous String/Comment Slice Scanning**
> Upgraded `TypeValue<'a>`, `Atom<'a>`, and `Expr<'a>` directly to use `Cow<'a, str>` (`Parser::parse_str_all` / `Parser::parse_str_one`) that borrow `&'a str` / `Cow<'a, str>` slices directly from the input string without allocating `String`s for symbols, keywords, comments, or unescaped strings. Added contiguous string/comment slice scanning in `SliceParser`.

| Benchmark Name                             | Result      | vs. Baseline           | Speedup    |
|--------------------------------------------|------------:|-----------------------:|-----------:|
| `bench_parse_borrowed_small` *(zero-copy)* | `123.61 ns` | *-92.3% vs. baseline*  |  **13.0x** |
| `bench_parse_borrowed_realistic` *(0-copy)*| `564.63 ns` | *-92.1% vs. baseline*  |  **12.6x** |

**10/25/2026 - Step 6: `raw-data` Zero-Copy Parsing (`Data::from_expr`), `eq_ignore_ascii_case`, & `Data::write_to`**
> Updated `raw-data` (`FromStr::from_str` and `DataFile::new`) to parse via zero-copy `Expr<'a>` and construct `Data` directly without intermediate `Expr` allocations, replaced `s.to_lowercase()` in `is_nil_symbol`/`is_t_symbol` with zero-allocation `eq_ignore_ascii_case`, merged `MapData`'s two passes into one, and added single-buffer `write_to(&self, buf: &mut String)` to `Data`, `ExprData`, `ListData`, and `MapData`.

| Benchmark Name                         | Baseline (Pre-Step 6) | Result                | vs. Baseline | Speedup  |
|----------------------------------------|----------------------:|----------------------:|-------------:|---------:|
| `bench_raw_data_from_str`              | `4.1091 µs` (4109 ns) | `1.7635 µs` (1764 ns) |   **-57.1%** | **2.3x** |
| `bench_raw_data_to_string`             | `3.6099 µs` (3610 ns) | `644.37 ns`           |   **-82.1%** | **5.6x** |

**10/25/2026 - Step 7: `serializer` & `deserializer` Zero-Allocation Case Conversion, Stack Integer Formatting, & Borrowed Visitor**
> Eliminated `convert_case::Casing::to_case` heap allocations on ASCII struct names, snake_case/kebab-case field names, and enum variants in `LispRPCSerializer` and `LispRPCDeserializer` using direct buffer writes, stack buffers (`[u8; 64]`), and `visitor.visit_borrowed_str`. Added stack-buffer `i64`/`u64` writers in `LispRPCSerializer`, eliminated `buf[..pos].to_vec()` in `lisp_rpc_to_str`, added an `AtomicBool` fast path for `GLOBAL_MAP_TYPES`, and eliminated command-name `String` allocation and response boxing in `RPCServer::handle`.

| Benchmark Name                         | Baseline (Pre-Step 7) | Result      | vs. Baseline | Speedup     |
|----------------------------------------|----------------------:|------------:|-------------:|------------:|
| `bench_serializer_to_str`              | `29.973 µs` (29973 ns)| `273.45 ns` |   **-99.1%** | **109.6x**  |
| `bench_deserializer_from_str`          | `21.701 µs` (21701 ns)| `380.50 ns` |   **-98.2%** |  **57.0x**  |

**10/26/2026 - Step 8: Unified `TypeValue<'a>` and End-to-End Zero-Copy `Data<'a>`**
> Parameterized dynamic raw-data structures (`Data<'a>`, `ExprData<'a>`, `ListData<'a>`, `MapData<'a>`, `DataMap<'a>`) with generic lifetime `'a`. `Data::from_expr` converts parsed `Expr<'a>` into dynamic `Data<'a>` zero-copy without reallocating symbols or keywords into `'static` `String`s.

| Benchmark Name                             | Previous Result       | New Result            | vs. Baseline            | Overall Speedup |
|--------------------------------------------|----------------------:|----------------------:|------------------------:|----------------:|
| `bench_raw_data_from_str`                  | `1.7635 µs` (1764 ns) | `1.1823 µs` (1182 ns) | **-71.2%** (vs 4.109 µs)| **3.5x**        |
| `bench_raw_data_to_string`                 | `644.37 ns`           | `654.22 ns`           | **-81.9%** (vs 3.610 µs)| **5.5x**        |
| `bench_parse_borrowed_small` *(zero-copy)* | `123.61 ns`           | `122.78 ns`           | **-92.3%** (vs 1.604 µs)| **13.1x**       |
| `bench_parse_borrowed_realistic` *(0-copy)*| `564.63 ns`           | `548.96 ns`           | **-92.3%** (vs 7.113 µs)| **13.0x**       |
| `bench_serializer_to_str`                  | `273.45 ns`           | `273.85 ns`           | **-99.1%** (vs 29.97 µs)| **109.5x**      |
| `bench_deserializer_from_str`              | `380.50 ns`           | `383.12 ns`           | **-98.2%** (vs 21.70 µs)| **56.6x**       |
| `bench_parse_schema_defs`                  | `3.7945 µs`           | `3.8224 µs`           | **-63.4%** (vs 10.43 µs)| **2.7x**        |
| `bench_parse_batch_20`                     | `50.084 µs`           | `50.137 µs`           | **-65.5%** (vs 145.3 µs)| **2.9x**        |
| `bench_stream_parser_chunked_32b`          | `1.8677 µs`           | `2.0791 µs`           | **-93.8%** (vs 33.39 µs)| **16.1x**       |
| `bench_expr_to_string`                     | `346.06 ns`           | `357.00 ns`           | **-87.6%** (vs 2.885 µs)| **8.1x**        |




