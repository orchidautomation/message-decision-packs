import {
  AbsoluteFill,
  Easing,
  Sequence,
  interpolate,
  useCurrentFrame,
  useVideoConfig,
} from "remotion";

type Scene = {
  readonly title: string;
  readonly eyebrow: string;
  readonly subtitle: string;
  readonly accent: string;
  readonly duration: number;
  readonly visual: "messy" | "pack" | "runner" | "gates" | "review" | "guardrail" | "closing";
};

const scenes: Scene[] = [
  {
    eyebrow: "01 / messy intake",
    title: "Start with rough proposal source files",
    subtitle: "OCR text, capture notes, proof inventory, and a rough compliance matrix stay local and synthetic for the demo.",
    accent: "messy-sources/",
    duration: 150,
    visual: "messy",
  },
  {
    eyebrow: "02 / pack creation",
    title: "Use Message Decision Packs to create the .mdp",
    subtitle: "The proposal template creates roles, requirements, proof, boundaries, review gates, evals, and the normalize-opportunity prompt.",
    accent: "mdp init --template proposal",
    duration: 150,
    visual: "pack",
  },
  {
    eyebrow: "03 / runner boundary",
    title: "The local runner stages normalization",
    subtitle: "Default mock mode proves the artifact chain without claiming model isolation. Real pilots need native/headless evidence.",
    accent: "mdp-proposal-runner.mjs",
    duration: 150,
    visual: "runner",
  },
  {
    eyebrow: "04 / CLI proof gates",
    title: "The CLI proves the local artifacts",
    subtitle: "Prompt output, source audit, validation, runner audit, and pack manifest are hash-bound into a receipt. Mock evidence blocks audit-grade.",
    accent: "mdp run-receipt --require-runner-audit",
    duration: 165,
    visual: "gates",
  },
  {
    eyebrow: "05 / review output",
    title: "Generate bounded proposal review support",
    subtitle: "Route the right cards, compile proof-output, verify bindings, and render a human-readable review layer.",
    accent: "verify-output --readable",
    duration: 150,
    visual: "review",
  },
  {
    eyebrow: "06 / guardrails",
    title: "Unsupported claims stay blocked",
    subtitle: "Missing certification, compliance status, past performance, pricing, or approval becomes a gap — not confident proposal prose.",
    accent: "CMMC compliant → needs revision",
    duration: 150,
    visual: "guardrail",
  },
  {
    eyebrow: "client video talk track",
    title: "MDP stores decision context. The CLI proves it.",
    subtitle: "Use mock mode for workshops; the CLI blocks it from audit-grade. Require real native/headless evidence before calling a pilot audit-grade.",
    accent: "messy files → .mdp → local runner → CLI receipt → review artifact",
    duration: 135,
    visual: "closing",
  },
];

const totalBefore = (index: number) => scenes.slice(0, index).reduce((sum, scene) => sum + scene.duration, 0);

const colors = {
  bg: "#060713",
  panel: "rgba(255,255,255,0.08)",
  panelStrong: "rgba(255,255,255,0.13)",
  text: "#F8FAFC",
  muted: "#A7B0C5",
  line: "rgba(255,255,255,0.16)",
  cyan: "#50E3FF",
  purple: "#9B7CFF",
  green: "#72F2A6",
  amber: "#FFD166",
  red: "#FF6B7A",
};

export const ProposalFlowVideo: React.FC = () => {
  const frame = useCurrentFrame();
  const { durationInFrames } = useVideoConfig();

  return (
    <AbsoluteFill style={{ backgroundColor: colors.bg, fontFamily: "Inter, ui-sans-serif, system-ui, sans-serif" }}>
      <Background />
      <Progress frame={frame} duration={durationInFrames} />
      {scenes.map((scene, index) => (
        <Sequence
          key={scene.visual}
          from={totalBefore(index)}
          durationInFrames={scene.duration}
          premountFor={30}
        >
          <SceneFrame scene={scene} index={index} />
        </Sequence>
      ))}
    </AbsoluteFill>
  );
};

const Background: React.FC = () => {
  const frame = useCurrentFrame();
  return (
    <AbsoluteFill>
      <div
        style={{
          position: "absolute",
          inset: 0,
          background:
            "radial-gradient(circle at 18% 22%, rgba(80, 227, 255, 0.24), transparent 32%), radial-gradient(circle at 82% 18%, rgba(155, 124, 255, 0.24), transparent 28%), linear-gradient(135deg, #060713 0%, #11142A 55%, #090B17 100%)",
        }}
      />
      <div
        style={{
          position: "absolute",
          inset: 0,
          opacity: 0.22,
          backgroundImage:
            "linear-gradient(rgba(255,255,255,0.08) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.08) 1px, transparent 1px)",
          backgroundSize: "72px 72px",
          translate: `${interpolate(frame, [0, 1050], [0, -72], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}px ${interpolate(frame, [0, 1050], [0, -36], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}px`,
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 620,
          height: 620,
          borderRadius: 999,
          background: "rgba(80, 227, 255, 0.12)",
          filter: "blur(80px)",
          right: -160,
          bottom: -220,
          scale: interpolate(frame, [0, 1050], [0.86, 1.16], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
        }}
      />
    </AbsoluteFill>
  );
};

const Progress: React.FC<{ readonly frame: number; readonly duration: number }> = ({ frame, duration }) => {
  return (
    <div style={{ position: "absolute", left: 90, right: 90, bottom: 54, height: 4, background: "rgba(255,255,255,0.13)", borderRadius: 999 }}>
      <div
        style={{
          width: `${interpolate(frame, [0, duration], [0, 100], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}%`,
          height: "100%",
          borderRadius: 999,
          background: `linear-gradient(90deg, ${colors.cyan}, ${colors.purple}, ${colors.green})`,
        }}
      />
    </div>
  );
};

const SceneFrame: React.FC<{ readonly scene: Scene; readonly index: number }> = ({ scene, index }) => {
  const frame = useCurrentFrame();
  const opacity = interpolate(frame, [0, 22, scene.duration - 18, scene.duration], [0, 1, 1, 0], {
    easing: Easing.bezier(0.16, 1, 0.3, 1),
    extrapolateLeft: "clamp",
    extrapolateRight: "clamp",
  });

  return (
    <AbsoluteFill style={{ opacity, padding: "92px 104px 106px" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 48 }}>
        <div style={{ color: colors.muted, fontSize: 30, letterSpacing: 3, textTransform: "uppercase", fontWeight: 800 }}>
          {scene.eyebrow}
        </div>
        <div style={{ color: colors.text, fontSize: 28, fontWeight: 800, opacity: 0.8 }}>Message Decision Packs</div>
      </div>
      <div style={{ display: "grid", gridTemplateColumns: "1.02fr 0.98fr", gap: 66, alignItems: "center", height: 770 }}>
        <div>
          <Badge label={scene.accent} color={index % 2 === 0 ? colors.cyan : colors.green} />
          <h1 style={{ color: colors.text, fontSize: 92, lineHeight: 0.98, letterSpacing: -4.5, margin: "28px 0 28px", maxWidth: 900 }}>
            {scene.title}
          </h1>
          <p style={{ color: colors.muted, fontSize: 38, lineHeight: 1.24, margin: 0, maxWidth: 820 }}>
            {scene.subtitle}
          </p>
        </div>
        <Visual scene={scene} />
      </div>
    </AbsoluteFill>
  );
};

const Badge: React.FC<{ readonly label: string; readonly color: string }> = ({ label, color }) => {
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 14, padding: "13px 20px", borderRadius: 999, background: "rgba(255,255,255,0.09)", border: `1px solid ${colors.line}` }}>
      <span style={{ width: 13, height: 13, borderRadius: 999, background: color, boxShadow: `0 0 24px ${color}` }} />
      <span style={{ color: colors.text, fontSize: 27, fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace" }}>{label}</span>
    </div>
  );
};

const Visual: React.FC<{ readonly scene: Scene }> = ({ scene }) => {
  switch (scene.visual) {
    case "messy":
      return <MessySources />;
    case "pack":
      return <PackCreation />;
    case "runner":
      return <RunnerBoundary />;
    case "gates":
      return <CliGates />;
    case "review":
      return <ReviewArtifact />;
    case "guardrail":
      return <Guardrail />;
    case "closing":
      return <ClosingFlow />;
  }
};

const Panel: React.FC<{ readonly children: React.ReactNode; readonly style?: React.CSSProperties }> = ({ children, style }) => (
  <div style={{ border: `1px solid ${colors.line}`, background: colors.panel, borderRadius: 34, boxShadow: "0 30px 80px rgba(0,0,0,0.28)", ...style }}>
    {children}
  </div>
);

const Pill: React.FC<{ readonly children: React.ReactNode; readonly color?: string }> = ({ children, color = colors.cyan }) => (
  <div style={{ padding: "14px 18px", borderRadius: 18, background: "rgba(255,255,255,0.08)", border: `1px solid ${colors.line}`, color, fontSize: 25, fontWeight: 800 }}>
    {children}
  </div>
);

const MessySources: React.FC = () => {
  const frame = useCurrentFrame();
  const files = ["01-rfp-ocr.txt", "02-capture-notes.md", "03-proof-inventory.md", "04-compliance-matrix.csv"];
  return (
    <Panel style={{ padding: 34 }}>
      <div style={{ color: colors.text, fontSize: 30, fontWeight: 900, marginBottom: 24 }}>messy-sources/</div>
      <div style={{ display: "grid", gap: 18 }}>
        {files.map((file, index) => (
          <div
            key={file}
            style={{
              opacity: interpolate(frame, [index * 10, index * 10 + 22], [0, 1], { easing: Easing.bezier(0.16, 1, 0.3, 1), extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
              translate: `${interpolate(frame, [index * 10, index * 10 + 22], [60, 0], { easing: Easing.bezier(0.16, 1, 0.3, 1), extrapolateLeft: "clamp", extrapolateRight: "clamp" })}px 0`,
              padding: "22px 24px",
              borderRadius: 22,
              background: index === 0 ? "rgba(255,209,102,0.14)" : colors.panelStrong,
              border: `1px solid ${index === 0 ? "rgba(255,209,102,0.45)" : colors.line}`,
              color: colors.text,
              fontSize: 30,
              fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
            }}
          >
            {file}
          </div>
        ))}
      </div>
      <div style={{ marginTop: 26, color: colors.amber, fontSize: 25, lineHeight: 1.25 }}>OCR typos + assumptions + missing proof stay visible.</div>
    </Panel>
  );
};

const PackCreation: React.FC = () => {
  const frame = useCurrentFrame();
  const items = ["roles", "requirements", "proof", "boundaries", "review gates", "evals"];
  return (
    <Panel style={{ padding: 36 }}>
      <TerminalLine command="mdp init --template proposal --dir pack" />
      <div style={{ height: 30 }} />
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 18 }}>
        {items.map((item, index) => (
          <div
            key={item}
            style={{
              opacity: interpolate(frame, [20 + index * 8, 42 + index * 8], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
              scale: interpolate(frame, [20 + index * 8, 42 + index * 8], [0.92, 1], { easing: Easing.bezier(0.34, 1.56, 0.64, 1), extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
            }}
          >
            <Pill color={index % 2 === 0 ? colors.green : colors.cyan}>.mdp/{item}</Pill>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 30, color: colors.muted, fontSize: 27 }}>Pack validates before any review output is trusted.</div>
    </Panel>
  );
};

const RunnerBoundary: React.FC = () => {
  const steps = ["declared inputs", "strict JSON", "source audit", "runner audit"];
  return (
    <Panel style={{ padding: 36 }}>
      <div style={{ display: "grid", gap: 18 }}>
        {steps.map((step, index) => (
          <FlowRow key={step} index={index} label={step} />
        ))}
      </div>
      <div style={{ marginTop: 30, padding: "20px 22px", borderRadius: 20, background: "rgba(255,209,102,0.12)", color: colors.amber, fontSize: 25, lineHeight: 1.28 }}>
        Demo mock mode is blocked from audit-grade. Production needs real native/headless runner evidence.
      </div>
    </Panel>
  );
};

const CliGates: React.FC = () => {
  const gates = [
    ["validate-prompt-output", "refs resolve"],
    ["fit", "insufficient-context visible"],
    ["run-receipt", "blocked unless verified"],
    ["route", "load the right cards"],
  ];
  return (
    <Panel style={{ padding: 36 }}>
      <div style={{ display: "grid", gap: 18 }}>
        {gates.map(([cmd, result], index) => (
          <div key={cmd} style={{ display: "grid", gridTemplateColumns: "1fr 0.9fr", gap: 16, alignItems: "center" }}>
            <TerminalLine command={`mdp ${cmd}`} delay={index * 8} />
            <Pill color={colors.green}>✓ {result}</Pill>
          </div>
        ))}
      </div>
    </Panel>
  );
};

const ReviewArtifact: React.FC = () => {
  return (
    <Panel style={{ padding: 34 }}>
      <div style={{ color: colors.text, fontSize: 34, fontWeight: 950, marginBottom: 22 }}>proposal-review.md</div>
      <ReviewLine label="Verification" value="proof-safe" color={colors.green} />
      <ReviewLine label="Requirement" value="Online intake" color={colors.cyan} />
      <ReviewLine label="Proof" value="synthetic rollout + training readiness" color={colors.purple} />
      <ReviewLine label="Gap" value="no approved certification proof" color={colors.amber} />
      <div style={{ marginTop: 26, color: colors.muted, fontSize: 25 }}>Readable layer only. Machine source of truth stays JSON.</div>
    </Panel>
  );
};

const Guardrail: React.FC = () => {
  const frame = useCurrentFrame();
  return (
    <Panel style={{ padding: 38, background: "rgba(255,107,122,0.08)" }}>
      <div style={{ color: colors.red, fontSize: 34, fontWeight: 950, marginBottom: 22 }}>Unsupported claim check</div>
      <div style={{ padding: 26, borderRadius: 24, background: "rgba(0,0,0,0.26)", border: `1px solid rgba(255,107,122,0.36)`, color: colors.text, fontSize: 37, lineHeight: 1.18 }}>
        “The sample team is CMMC compliant.”
      </div>
      <div
        style={{
          marginTop: 28,
          opacity: interpolate(frame, [36, 62], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
          translate: `0 ${interpolate(frame, [36, 62], [32, 0], { extrapolateLeft: "clamp", extrapolateRight: "clamp" })}px`,
          color: colors.red,
          fontSize: 40,
          fontWeight: 950,
        }}
      >
        blocked → needs revision
      </div>
      <div style={{ marginTop: 20, color: colors.muted, fontSize: 26 }}>The gap remains explicit instead of becoming proposal prose.</div>
    </Panel>
  );
};

const ClosingFlow: React.FC = () => {
  const items = ["messy files", ".mdp", "local runner", "CLI receipt", "review artifact"];
  return (
    <Panel style={{ padding: 38 }}>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 14, alignItems: "center" }}>
        {items.map((item, index) => (
          <div key={item} style={{ display: "contents" }}>
            <Pill color={index === items.length - 1 ? colors.green : colors.cyan}>{item}</Pill>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 34, color: colors.text, fontSize: 42, fontWeight: 900, lineHeight: 1.1 }}>Walk the client through what is sourced, what is proved, and what remains a human review decision.</div>
    </Panel>
  );
};

const FlowRow: React.FC<{ readonly index: number; readonly label: string }> = ({ index, label }) => {
  const frame = useCurrentFrame();
  return (
    <div style={{ display: "grid", gridTemplateColumns: "90px 1fr", alignItems: "center", gap: 20 }}>
      <div style={{ width: 64, height: 64, borderRadius: 999, display: "grid", placeItems: "center", color: colors.bg, background: index % 2 === 0 ? colors.cyan : colors.green, fontSize: 28, fontWeight: 950 }}>
        {index + 1}
      </div>
      <div
        style={{
          opacity: interpolate(frame, [index * 12, index * 12 + 24], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
          translate: `${interpolate(frame, [index * 12, index * 12 + 24], [46, 0], { easing: Easing.bezier(0.16, 1, 0.3, 1), extrapolateLeft: "clamp", extrapolateRight: "clamp" })}px 0`,
        }}
      >
        <Pill color={colors.text}>{label}</Pill>
      </div>
    </div>
  );
};

const TerminalLine: React.FC<{ readonly command: string; readonly delay?: number }> = ({ command, delay = 0 }) => {
  const frame = useCurrentFrame();
  return (
    <div
      style={{
        opacity: interpolate(frame, [delay, delay + 18], [0, 1], { extrapolateLeft: "clamp", extrapolateRight: "clamp" }),
        padding: "18px 22px",
        borderRadius: 18,
        background: "rgba(0,0,0,0.34)",
        border: `1px solid ${colors.line}`,
        color: colors.green,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: 24,
        whiteSpace: "nowrap",
      }}
    >
      $ {command}
    </div>
  );
};

const ReviewLine: React.FC<{ readonly label: string; readonly value: string; readonly color: string }> = ({ label, value, color }) => (
  <div style={{ display: "grid", gridTemplateColumns: "220px 1fr", gap: 18, padding: "18px 0", borderBottom: `1px solid ${colors.line}` }}>
    <div style={{ color, fontSize: 25, fontWeight: 900 }}>{label}</div>
    <div style={{ color: colors.text, fontSize: 28, lineHeight: 1.18 }}>{value}</div>
  </div>
);
