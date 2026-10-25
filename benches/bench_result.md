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
