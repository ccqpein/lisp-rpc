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


