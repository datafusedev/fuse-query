// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

#[cfg(test)]
mod tests {
    use common_exception::Result;
    use crate::optimizers::optimizer_scatters::ScattersOptimizer;
    use crate::optimizers::IOptimizer;
    use crate::sql::PlanParser;
    use crate::clusters::{Cluster, Node};
    use crate::configs::Config;
    use std::sync::Arc;

    #[test]
    fn test_scatter_optimizer() -> Result<()> {
        #[allow(dead_code)]
        struct Test {
            name: &'static str,
            query: &'static str,
            expect: &'static str,
        }

        let tests = vec![
            Test {
                name: "Scalar query",
                query: "SELECT 1",
                expect: "\
                Projection: 1:UInt64\
                \n  Expression: 1:UInt8 (Before Projection)\
                \n    ReadDataSource: scan partitions: [1], scan schema: [dummy:UInt8], statistics: [read_rows: 0, read_bytes: 0]",
            },
            Test {
                name: "Small local table query",
                query: "SELECT number FROM numbers_local(100)",
                expect: "\
                Projection: number:UInt64\
                \n  Expression: number:UInt64 (Before Projection)\
                \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
            },
            Test {
                name: "Small local table aggregate query with group by key",
                query: "SELECT SUM(number) FROM numbers_local(100) GROUP BY number % 3",
                expect: "\
                AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n  AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
            },
            Test {
                name: "Small local table aggregate query with group by keys",
                query: "SELECT SUM(number) FROM numbers_local(100) GROUP BY number % 3, number % 2",
                expect: "\
                AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n  AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
            },
            Test {
                name: "Small local table aggregate query without group by",
                query: "SELECT SUM(number) FROM numbers_local(100)",
                expect: "\
                AggregatorFinal: groupBy=[[]], aggr=[[SUM([number])]]\
                \n  AggregatorPartial: groupBy=[[]], aggr=[[SUM([number])]]\
                \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
            },
            Test {
                name: "Large local table query",
                query: "SELECT number FROM numbers_local(100000000)",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  Projection: number:UInt64\
                \n  Expression: number:UInt64 (Before Projection)\
                \n    RedistributeStage[expr: blockNumber([])]\
                \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large local table aggregate query with group by key",
                query: "SELECT SUM(number) FROM numbers_local(100000000) GROUP BY number % 3",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: (number % 3)]\
                \n    AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n      RedistributeStage[expr: blockNumber([])]\
                \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large local table aggregate query with group by keys",
                query: "SELECT SUM(number) FROM numbers_local(100000000) GROUP BY number % 3, number % 2",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: (number % 3)]\
                \n    AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n      RedistributeStage[expr: blockNumber([])]\
                \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large local table aggregate query without group by",
                query: "SELECT SUM(number) FROM numbers_local(100000000)",
                expect: "\
                AggregatorFinal: groupBy=[[]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: 0]\
                \n    AggregatorPartial: groupBy=[[]], aggr=[[SUM([number])]]\
                \n      RedistributeStage[expr: blockNumber([])]\
                \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large cluster table query",
                query: "SELECT number FROM numbers(100000000)",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  Projection: number:UInt64\
                \n  Expression: number:UInt64 (Before Projection)\
                \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large cluster table aggregate query with group by key",
                query: "SELECT SUM(number) FROM numbers(100000000) GROUP BY number % 3",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: (number % 3)]\
                \n    AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM([number])]]\
                \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large cluster table aggregate query with group by keys",
                query: "SELECT SUM(number) FROM numbers(100000000) GROUP BY number % 3, number % 2",
                expect: "\
                RedistributeStage[expr: 0]\
                \n  AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: (number % 3)]\
                \n    AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM([number])]]\
                \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            },
            Test {
                name: "Large cluster table aggregate query without group by",
                query: "SELECT SUM(number) FROM numbers(100000000)",
                expect: "\
                AggregatorFinal: groupBy=[[]], aggr=[[SUM([number])]]\
                \n  RedistributeStage[expr: 0]\
                \n    AggregatorPartial: groupBy=[[]], aggr=[[SUM([number])]]\
                \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
            }
        ];

        for test in tests {
            let ctx = crate::tests::try_create_context()?;
            let cluster = Cluster::create(Config::default());
            cluster.add_node(&Node {
                name: String::from("dummy"),
                priority: 1,
                address: String::from("dummy"),
                local: false,
            });

            ctx.with_cluster(cluster.clone());
            let plan = PlanParser::create(ctx.clone()).build_from_sql(test.query)?;

            let mut optimizer = ScattersOptimizer::create(ctx);
            let optimized = optimizer.optimize(&plan)?;
            let actual = format!("{:?}", optimized);
            assert_eq!(test.expect, actual, "{:#?}", test.name);
        }

        Ok(())
    }
}