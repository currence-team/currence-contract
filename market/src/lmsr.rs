pub fn fund(liquidity: f64, n: usize) -> f64 {
    liquidity * (n as f64).ln()
}

pub fn liquidity(fund: f64, n: usize) -> f64 {
    fund / (n as f64).ln()
}

fn coefficient(liquidity: f64, volumes: &[f64]) -> (Vec<f64>, f64) {
    let mut max: f64 = 0.0;

    let ret = volumes
        .iter()
        .map(|v| {
            let tmp = v / liquidity;
            if tmp > max {
                max = tmp;
            }
            tmp
        })
        .collect::<Vec<_>>();

    (ret, max)
}

fn shift_exp_sum(max: f64, data: &[f64]) -> f64 {
    data.iter().fold(0.0, |acc, &v| acc + (v - max).exp())
}

fn ln_sum(liquidity: f64, volumes: &[f64]) -> f64 {
    let (tmp, max) = coefficient(liquidity, volumes);

    let sum = shift_exp_sum(max, &tmp);
    sum.ln() + max
}

pub fn cost(liquidity: f64, volumes: &[f64]) -> f64 {
    liquidity * ln_sum(liquidity, volumes)
}

pub fn compute_price(liquidity: f64, volumes: &[f64]) -> Vec<f64> {
    let (tmp, max) = coefficient(liquidity, volumes);
    let sum = shift_exp_sum(max, &tmp);

    tmp.iter()
        .map(|v| (v - max - sum.ln()).exp())
        .collect::<Vec<_>>()
}

pub fn estimate(liquidity: f64, origin: &[f64], index: usize, amount: f64) -> f64 {
    let mut after = Vec::from(origin);
    after[index] += amount;
    cost(liquidity, &after) - cost(liquidity, origin)
}

pub fn volume(liquidity: f64, origin: &[f64], index: usize, amount: f64) -> f64 {
    let a = (amount / liquidity).exp() - 1.0;

    let (tmp, max) = coefficient(liquidity, origin);
    let sum = shift_exp_sum(max, &tmp);

    let p = (tmp[index] - max - sum.ln()).exp();

    liquidity * (a / p + 1.0).ln()
}
#[cfg(test)]
mod tests {
    use super::*;

    const FUND: f64 = 69.314_718_055_994_53;
    const LIQ: f64 = 100.0;

    fn test_round(
        vols: &[f64],
        index: usize,
        add: f64,
        old_cost: f64,
        ans: &[f64],
    ) -> (Vec<f64>, f64) {
        let mut ret = Vec::from(vols);
        ret[index] += add;

        let prices = compute_price(LIQ, &ret);
        let new_cost = cost(LIQ, &ret);
        let amount = new_cost - old_cost;

        assert_eq!(prices[0], ans[0]);
        assert_eq!(prices[1], ans[1]);
        assert_eq!(new_cost, ans[2]);
        assert_eq!(amount, ans[3]);

        let test_vol = volume(LIQ, vols, index, amount);
        assert!((test_vol - add).abs() < 0.000_000_000_01);
        (ret, new_cost)
    }

    #[test]
    fn price_increases() {
        let volumes: [f64; 2] = [0.0, 0.0];
        let est = estimate(50.0, &volumes, 1, 10.0);
        assert!(est > 5.2);
    }

    #[test]
    fn it_works() {
        assert_eq!(FUND, fund(LIQ, 2));
        assert_eq!(LIQ, liquidity(FUND, 2));

        let volumes: [f64; 2] = [0.0, 0.0];

        // initialize
        let prices = compute_price(LIQ, &volumes);
        let cost = cost(LIQ, &volumes);

        assert_eq!(prices[0], 0.5);
        assert_eq!(prices[1], 0.5);
        assert_eq!(cost, FUND);

        // 1st
        /*
        let add: f64 = 100.0;
        volumes[0] += add;
        let prices = compute_price(LIQ, &volumes);
        let cost1 = cost(LIQ, &volumes);
        let amount = cost1 - cost0;

        assert_eq!(prices[0], 0.731_058_578_630_004_9);
        assert_eq!(prices[1], 0.268_941_421_369_995_1);
        assert_eq!(cost1, 131.326_168_751_822_28);
        assert_eq!(amount, 62.011_450_695_827_75);

        volumes[0] -= add;
        let vol = volume(LIQ, &volumes, 0, amount);
        assert!((vol - add).abs() < 0.00000001);

        volumes[0] += add;
        */

        let (volumes, cost) = test_round(
            &volumes,
            0,
            100.0,
            cost,
            &[
                0.731_058_578_630_004_9,
                0.268_941_421_369_995_1,
                131.326_168_751_822_28,
                62.011_450_695_827_75,
            ],
        );

        // 2nd
        let (volumes, cost) = test_round(
            &volumes,
            0,
            40.0,
            cost,
            &[
                0.802_183_888_558_581_7,
                0.197_816_111_441_418_25,
                162.041_740_991_845_1,
                30.715_572_240_022_823,
            ],
        );

        // 3rd
        let (volumes, cost) = test_round(
            &volumes,
            1,
            20.0,
            cost,
            &[
                0.768_524_783_499_017_5,
                0.231_475_216_500_982_32,
                166.328_246_733_803_1,
                4.286_505_741_957_995_5,
            ],
        );

        // 4th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            50.0,
            cost,
            &[
                0.845_534_734_916_465_2,
                0.154_465_265_083_534_73,
                206.778_602_938_626_6,
                40.450_356_204_823_49,
            ],
        );

        // 5th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            100.0,
            cost,
            &[
                0.937_026_643_943_003_5,
                0.062_973_356_056_996_53,
                296.504_356_177_659,
                89.725_753_239_032_43,
            ],
        );

        // 6th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            50.0,
            cost,
            &[
                0.900_249_510_880_314_8,
                0.099_750_489_119_685_12,
                300.508_331_976_869_6,
                4.003_975_799_210_593,
            ],
        );

        // 7th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            -40.0,
            cost,
            &[
                0.858_148_935_099_512_3,
                0.141_851_064_900_487_8,
                265.297_761_052_607_4,
                -35.210_570_924_262_186,
            ],
        );

        // 8th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            30.0,
            cost,
            &[
                0.817_574_476_193_643_7,
                0.182_425_523_806_356_35,
                270.141_327_798_275_26,
                4.843_566_745_667_829,
            ],
        );

        // 9th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            40.0,
            cost,
            &[
                0.869_891_525_637_002_1,
                0.130_108_474_362_997_88,
                303.938_675_828_296,
                33.797_348_030_020_77,
            ],
        );

        // 10th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            300.0,
            cost,
            &[
                0.249_739_894_404_882_4,
                0.750_260_105_595_117_7,
                428.733_532_511_543_1,
                124.794_856_683_247_08,
            ],
        );

        // 11th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            -10.0,
            cost,
            &[
                0.268_941_421_369_995_1,
                0.731_058_578_630_004_9,
                421.326_168_751_822_3,
                -7.407_363_759_720_795,
            ],
        );

        // 12th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            150.0,
            cost,
            &[
                0.075_858_180_021_243_52,
                0.924_141_819_978_756_6,
                547.888_973_429_255,
                126.562_804_677_432_67,
            ],
        );

        // 13th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            -40.0,
            cost,
            &[
                0.052_153_563_078_417_72,
                0.947_846_436_921_582_3,
                545.356_277_621_796_4,
                -2.532_695_807_458_594_6,
            ],
        );

        // 14th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            20.0,
            cost,
            &[
                0.043_107_254_941_086_144,
                0.956_892_745_058_913_9,
                564.406_396_793_857_4,
                19.050_119_172_061_045,
            ],
        );

        // 15th
        let (volumes, cost) = test_round(
            &volumes,
            0,
            40.0,
            cost,
            &[
                0.062_973_356_056_996_53,
                0.937_026_643_943_003_5,
                566.504_356_177_659,
                2.097_959_383_801_594_4,
            ],
        );

        // 16th
        let (volumes, cost) = test_round(
            &volumes,
            1,
            200.0,
            cost,
            &[
                0.009_013_298_652_847_833,
                0.990_986_701_347_152,
                760.905_416_416_988_7,
                194.401_060_239_329_7,
            ],
        );

        let (_volumes, _cost) = test_round(
            &volumes,
            1,
            -100.0,
            cost,
            &[
                0.024_127_021_417_669_214,
                0.975_872_978_582_330_8,
                662.442_284_593_378,
                -98.463_131_823_610_75,
            ],
        );
    }
}
