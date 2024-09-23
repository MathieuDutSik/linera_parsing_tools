# linera_parsing_tools
Some tools and manuals related to linera-protocol

## Reading data from the Prometheus service

The tool
```
parsing_prometheus_run
```
allows to parse the variables from the run in a specific time interval.

## Running and obtaining the metrics

The tool is
```
run_and_obtain_metrics
```
allows to do the full set of operations from the compilation till the run.
WARNING: When running it, no other linera process can run locally


## Formatting and linting the source code

Make sure to fix the lint errors reported by
```
cargo clippy --all-targets --all-features
```
and format the code with
```
cargo +nightly fmt
```

