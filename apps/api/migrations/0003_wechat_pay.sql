-- 微信支付 JSAPI：商户单号、微信交易号、冻结金额（分）和支付渠道。
-- 入账只发生在 paid 状态翻转时；充值流水按订单号唯一，避免重复加积分。

ALTER TABLE credit_orders
    ADD COLUMN out_trade_no TEXT,
    ADD COLUMN transaction_id TEXT,
    ADD COLUMN prepay_id TEXT,
    ADD COLUMN payer_openid TEXT,
    ADD COLUMN amount_fen INTEGER,
    ADD COLUMN pay_channel TEXT;

UPDATE credit_orders
SET amount_fen = price_cents
WHERE amount_fen IS NULL;

UPDATE credit_orders
SET out_trade_no = replace(id::text, '-', '')
WHERE out_trade_no IS NULL;

UPDATE credit_orders
SET pay_channel = 'mock'
WHERE pay_channel IS NULL;

ALTER TABLE credit_orders
    ALTER COLUMN amount_fen SET NOT NULL,
    ALTER COLUMN out_trade_no SET NOT NULL,
    ALTER COLUMN pay_channel SET NOT NULL;

ALTER TABLE credit_orders
    ADD CONSTRAINT credit_orders_pay_channel_chk
        CHECK (pay_channel IN ('mock', 'wechat'));

CREATE UNIQUE INDEX credit_orders_out_trade_no_key ON credit_orders (out_trade_no);

CREATE UNIQUE INDEX credit_orders_transaction_id_key ON credit_orders (transaction_id)
    WHERE transaction_id IS NOT NULL;

CREATE UNIQUE INDEX credit_ledger_recharge_once ON credit_ledger (reason, ref_id)
    WHERE reason = 'recharge' AND ref_id IS NOT NULL;
