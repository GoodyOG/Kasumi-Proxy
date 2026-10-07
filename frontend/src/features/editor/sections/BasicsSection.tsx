import { Field, SectionLabel, Select } from "../../../components";
import type { Profile, Protocol } from "../../../generated/bindings";
import { PROTOCOL_OPTS } from "../../../generated/defaults";
import { useT } from "../../../i18n";
import type { EndpointSetter, FieldErrors, MetaSetter } from "../types";

const PROTOCOL_LABELS: Record<Protocol, string> = {
  vless: "VLESS",
  vmess: "VMess",
  trojan: "Trojan",
  shadowsocks: "Shadowsocks",
  socks: "SOCKS",
  http: "HTTP",
  wireguard: "WireGuard",
  hysteria2: "Hysteria2",
  tuic: "TUIC",
  anytls: "AnyTLS",
  naive: "Naive",
  shadowtls: "ShadowTLS",
  custom: "Custom config",
};

export function BasicsSection({
  draft,
  setMeta,
  setEndpoint,
  errors,
  groupOpts,
  changeProtocol,
}: {
  draft: Profile;
  setMeta: MetaSetter;
  setEndpoint: EndpointSetter;
  errors: FieldErrors;
  groupOpts: Array<{ value: string; label: string }>;
  changeProtocol: (proto: Protocol) => void;
}) {
  const t = useT();

  return (
    <>
      <SectionLabel>{t("editor.basics")}</SectionLabel>
      <Select
        label={t("editor.protocol")}
        value={draft.protocol}
        options={PROTOCOL_OPTS.map((protocol) => ({
          value: protocol,
          label: PROTOCOL_LABELS[protocol],
        }))}
        onChange={(value) => changeProtocol(value as Protocol)}
      />
      <Field
        label={t("editor.remarks")}
        mono={false}
        value={draft.meta.remarks}
        onChange={(value) => setMeta({ remarks: value })}
        error={errors.remarks}
      />

      {draft.protocol !== "custom" && (
        <div className="input-row" style={{ marginBottom: 14 }}>
          <Field
            label={t("editor.address")}
            value={draft.endpoint.address}
            onChange={(value) => setEndpoint({ address: value })}
            error={errors.address}
          />
          <div style={{ width: 96, flex: "0 0 auto" }}>
            <Field
              label={t("editor.port")}
              type="number"
              value={draft.endpoint.port}
              onChange={(value) => setEndpoint({ port: Number(value) })}
              error={errors.port}
            />
          </div>
        </div>
      )}

      <Select
        label={t("editor.group")}
        value={draft.meta.groupId}
        options={groupOpts}
        onChange={(value) => setMeta({ groupId: value })}
      />
    </>
  );
}
