use std::collections::BTreeMap;

use criterion::{criterion_group, criterion_main, Criterion, ParameterizedBenchmark};
use lazy_static::lazy_static;
use rand::Rng;

use ced::datastruct::RBTree;

fn make_data(size: usize) -> Vec<i64> {
    let mut rng = rand::thread_rng();
    let low = -1 * (size as i64);
    let high = size as i64;
    let mut data = Vec::with_capacity(size);
    for _ in 0..size {
        data.push(rng.gen_range(low, high));
    }
    data
}

lazy_static! {
    static ref DATAS: BTreeMap<usize, Vec<i64>> = {
        let mut datas = BTreeMap::new();
        for i in &[10, 100, 500, 1_000] {
            datas.insert(*i, make_data(*i));
        }
        datas
    };
}

fn sv_insert(sv: &mut Vec<i64>, data: &[i64]) {
    for v in data {
        sv.push(*v);
        sv.sort();
    }
}

fn sv_contains(sv: &Vec<i64>, values: &[i64]) {
    for value in values {
        assert!(sv.contains(value));
    }
}

fn sv_delete(sv: &mut Vec<i64>, values: &[i64]) {
    for value in values {
        let index = sv.iter().position(|x| x == value).unwrap();
        sv.remove(index);
    }
}

fn rbt_insert(rbt: &mut RBTree<i64>, data: &[i64]) {
    for v in data {
        rbt.insert(*v);
    }
}

fn rbt_contains(rbt: &RBTree<i64>, values: &[i64]) {
    for value in values {
        assert!(rbt.contains(value));
    }
}

fn rbt_delete(rbt: &mut RBTree<i64>, values: &[i64]) {
    for value in values {
        rbt.remove_data(value);
    }
}

fn loads_of_values(c: &mut Criterion) {
    c.bench(
        "insert",
        ParameterizedBenchmark::new(
            "sorted vec",
            |b, s| {
                let mut sv = Vec::new();
                b.iter(|| sv_insert(&mut sv, &DATAS[s]));
            },
            DATAS.keys().map(|k| *k).collect::<Vec<usize>>(),
        )
        .with_function("rbtree", |b, s| {
            let mut rbt = RBTree::new();
            b.iter(|| rbt_insert(&mut rbt, &DATAS[s]));
        }),
    );
    c.bench(
        "contains",
        ParameterizedBenchmark::new(
            "sorted vec",
            |b, s| {
                let mut sv = Vec::new();
                sv_insert(&mut sv, &DATAS[s]);
                b.iter(|| sv_contains(&sv, &DATAS[s][..5]));
            },
            DATAS.keys().map(|k| *k).collect::<Vec<usize>>(),
        )
        .with_function("rbtree", |b, s| {
            let mut rbt = RBTree::new();
            rbt_insert(&mut rbt, &DATAS[s]);
            b.iter(|| rbt_contains(&rbt, &DATAS[s][..5]));
        }),
    );
    c.bench(
        "clone",
        ParameterizedBenchmark::new(
            "sorted vec",
            |b, s| {
                let mut sv = Vec::new();
                sv_insert(&mut sv, &DATAS[s]);
                b.iter(|| sv.clone());
            },
            DATAS.keys().map(|k| *k).collect::<Vec<usize>>(),
        )
        .with_function("rbtree", |b, s| {
            let mut rbt = RBTree::new();
            rbt_insert(&mut rbt, &DATAS[s]);
            b.iter(|| rbt.clone());
        }),
    );
    c.bench(
        "delete",
        ParameterizedBenchmark::new(
            "sorted vec",
            |b, s| {
                let mut sv = Vec::new();
                sv_insert(&mut sv, &DATAS[s]);
                b.iter(|| sv_delete(&mut sv.clone(), &DATAS[s][5..10]));
            },
            DATAS.keys().map(|k| *k).collect::<Vec<usize>>(),
        )
        .with_function("rbtree", |b, s| {
            let mut rbt = RBTree::new();
            rbt_insert(&mut rbt, &DATAS[s]);
            b.iter(|| rbt_delete(&mut rbt.clone(), &DATAS[s][5..10]));
        }),
    );
}

criterion_group!(benches, loads_of_values);
criterion_main!(benches);
