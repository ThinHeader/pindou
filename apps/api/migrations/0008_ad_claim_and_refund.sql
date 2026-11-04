-- 每日免费领取停用。daily_claims 改为激励广告奖励：每个用户每个东八区自然日一行。
COMMENT ON TABLE daily_claims IS
    '激励广告奖励幂等：每个用户每个东八区自然日一行。每日免费领取已停用。';

-- 虚拟支付客诉退款。退款成功后把订单标成 refunded，并只扣回一次积分。
DO $$
DECLARE
    cname text;
BEGIN
    FOR cname IN
        SELECT conname
        FROM pg_constraint
        WHERE conrelid = 'credit_orders'::regclass
          AND contype = 'c'
          AND pg_get_constraintdef(oid) ILIKE '%status%'
          AND pg_get_constraintdef(oid) NOT ILIKE '%pay_channel%'
    LOOP
        EXECUTE format('ALTER TABLE credit_orders DROP CONSTRAINT %I', cname);
    END LOOP;
END $$;

ALTER TABLE credit_orders
    ADD CONSTRAINT credit_orders_status_check
        CHECK (status IN ('created', 'paid', 'refunding', 'refunded'));

ALTER TABLE credit_orders
    ADD COLUMN refund_order_id TEXT,
    ADD COLUMN refunded_at TIMESTAMPTZ;

CREATE UNIQUE INDEX credit_orders_refund_order_id_key
    ON credit_orders (refund_order_id)
    WHERE refund_order_id IS NOT NULL;

CREATE UNIQUE INDEX credit_ledger_refund_once
    ON credit_ledger (reason, ref_id)
    WHERE reason = 'refund' AND ref_id IS NOT NULL;
