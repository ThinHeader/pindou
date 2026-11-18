-- 微信 session_key 只存密文，用来给虚拟支付做用户态签名。
-- 明文不落库，接口也不回给客户端。

ALTER TABLE users
    ADD COLUMN wx_session_key_enc TEXT;

ALTER TABLE credit_orders
    DROP CONSTRAINT credit_orders_pay_channel_chk;

ALTER TABLE credit_orders
    ADD CONSTRAINT credit_orders_pay_channel_chk
        CHECK (pay_channel IN ('mock', 'wechat', 'virtual'));
