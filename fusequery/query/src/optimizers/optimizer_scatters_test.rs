// Copyright 2020-2021 The Datafuse Authors.
//
// SPDX-License-Identifier: Apache-2.0.

use common_exception::Result;
use common_runtime::tokio;

use crate::optimizers::optimizer_scatters::ScattersOptimizer;
use crate::optimizers::Optimizer;
use crate::sql::PlanParser;
use crate::tests::try_create_cluster_context;
use crate::tests::ClusterNode;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn test_scatter_optimizer() -> Result<()> {
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
            Projection: 1:UInt8\
            \n  Expression: 1:UInt8 (Before Projection)\
            \n    ReadDataSource: scan partitions: [1], scan schema: [dummy:UInt8], statistics: [read_rows: 1, read_bytes: 1]",
        },
        Test {
            name: "Small local table query",
            query: "SELECT number FROM numbers_local(100)",
            expect: "\
            Projection: number:UInt64\
            \n  ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
        },
        Test {
            name: "Small local table aggregate query with group by key",
            query: "SELECT SUM(number) FROM numbers_local(100) GROUP BY number % 3",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n      Expression: (number % 3):UInt8, number:UInt64 (Before GroupBy)\
            \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
        },
        Test {
            name: "Small local table aggregate query with group by keys",
            query: "SELECT SUM(number) FROM numbers_local(100) GROUP BY number % 3, number % 2",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n      Expression: (number % 3):UInt8, (number % 2):UInt8, number:UInt64 (Before GroupBy)\
            \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
        },
        Test {
            name: "Small local table aggregate query without group by",
            query: "SELECT SUM(number) FROM numbers_local(100)",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[]], aggr=[[SUM(number)]]\
            \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100, read_bytes: 800]",
        },
        Test {
            name: "Large local table query",
            query: "SELECT number FROM numbers_local(100000000)",
            expect: "\
            Projection: number:UInt64\
            \n  ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large local table aggregate query with group by key",
            query: "SELECT SUM(number) FROM numbers_local(100000000) GROUP BY number % 3",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n      Expression: (number % 3):UInt8, number:UInt64 (Before GroupBy)\
            \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large local table aggregate query with group by keys",
            query: "SELECT SUM(number) FROM numbers_local(100000000) GROUP BY number % 3, number % 2",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n      Expression: (number % 3):UInt8, (number % 2):UInt8, number:UInt64 (Before GroupBy)\
            \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large local table aggregate query without group by",
            query: "SELECT SUM(number) FROM numbers_local(100000000)",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[]], aggr=[[SUM(number)]]\
            \n    AggregatorPartial: groupBy=[[]], aggr=[[SUM(number)]]\
            \n      ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large cluster table query",
            query: "SELECT number FROM numbers(100000000)",
            expect: "\
            RedistributeStage[expr: 0]\
            \n  Projection: number:UInt64\
            \n    ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large cluster table aggregate query with group by key",
            query: "SELECT SUM(number) FROM numbers(100000000) GROUP BY number % 3",
            expect: "\
            RedistributeStage[expr: 0]\
            \n  Projection: SUM(number):UInt64\
            \n    AggregatorFinal: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n      RedistributeStage[expr: sipHash(_group_by_key)]\
            \n        AggregatorPartial: groupBy=[[(number % 3)]], aggr=[[SUM(number)]]\
            \n          Expression: (number % 3):UInt8, number:UInt64 (Before GroupBy)\
            \n            ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large cluster table aggregate query with group by keys",
            query: "SELECT SUM(number) FROM numbers(100000000) GROUP BY number % 3, number % 2",
            expect: "\
            RedistributeStage[expr: 0]\
            \n  Projection: SUM(number):UInt64\
            \n    AggregatorFinal: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n      RedistributeStage[expr: sipHash(_group_by_key)]\
            \n        AggregatorPartial: groupBy=[[(number % 3), (number % 2)]], aggr=[[SUM(number)]]\
            \n          Expression: (number % 3):UInt8, (number % 2):UInt8, number:UInt64 (Before GroupBy)\
            \n            ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Large cluster table aggregate query without group by",
            query: "SELECT SUM(number) FROM numbers(100000000)",
            expect: "\
            Projection: SUM(number):UInt64\
            \n  AggregatorFinal: groupBy=[[]], aggr=[[SUM(number)]]\
            \n    RedistributeStage[expr: 0]\
            \n      AggregatorPartial: groupBy=[[]], aggr=[[SUM(number)]]\
            \n        ReadDataSource: scan partitions: [8], scan schema: [number:UInt64], statistics: [read_rows: 100000000, read_bytes: 800000000]",
        },
        Test {
            name: "Standalone query with standalone subquery",
            query: "SELECT * FROM numbers_local(1) WHERE EXISTS(SELECT * FROM numbers_local(1))",
            expect: "\
            Projection: number:UInt64\
            \n  Filter: exists(subquery(_subquery_1))\
            \n    Create sub queries sets: [_subquery_1]\
            \n      Projection: number:UInt64\
            \n        ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]\
            \n      ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]",
        },
        Test {
            name: "Standalone query with cluster subquery",
            query: "SELECT * FROM numbers_local(1) WHERE EXISTS(SELECT * FROM numbers(1))",
            expect: "Projection: number:UInt64\
            \n  Filter: exists(subquery(_subquery_1))\
            \n    Create sub queries sets: [_subquery_1]\
            \n      RedistributeStage[expr: 0]\
            \n        Projection: number:UInt64\
            \n          ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]\
            \n      ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]",
        },
        Test {
            name: "Cluster query with standalone subquery",
            query: "SELECT * FROM numbers(1) WHERE EXISTS(SELECT * FROM numbers_local(1))",
            expect: "\
            RedistributeStage[expr: 0]\
            \n  Projection: number:UInt64\
            \n    Filter: exists(subquery(_subquery_1))\
            \n      Create sub queries sets: [_subquery_1]\
            \n        Broadcast in cluster\
            \n          Projection: number:UInt64\
            \n            ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]\
            \n        ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]",
        },
        Test {
            name: "Cluster query with cluster subquery",
            query: "SELECT * FROM numbers(1) WHERE EXISTS(SELECT * FROM numbers(1))",
            expect: "\
            RedistributeStage[expr: 0]\
            \n  Projection: number:UInt64\
            \n    Filter: exists(subquery(_subquery_1))\
            \n      Create sub queries sets: [_subquery_1]\
            \n        Broadcast in cluster\
            \n          Projection: number:UInt64\
            \n            ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]\
            \n        ReadDataSource: scan partitions: [1], scan schema: [number:UInt64], statistics: [read_rows: 1, read_bytes: 8]",
        },
    ];

    for test in tests {
        let ctx = try_create_cluster_context(&vec![ClusterNode::create(
            "Github",
            1,
            "www.github.com:9090",
        )])?;

        let plan = PlanParser::create(ctx.clone()).build_from_sql(test.query)?;
        let mut optimizer = ScattersOptimizer::create(ctx);
        let optimized = optimizer.optimize(&plan)?;
        let actual = format!("{:?}", optimized);
        assert_eq!(test.expect, actual, "{:#?}", test.name);
    }

    Ok(())
}
