import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { usePrefs } from "../lib/prefs";
import { THEMES } from "../lib/theme";
import { greetingFor, HOUR_LABELS } from "../utils/format";
import { call } from "../utils/ipc";
import { shortcutParts } from "../utils/shortcut";
import { BarChart } from "../ui/BarChart";
import { Icon } from "../ui/Icon";

type Step = "welcome" | "nick" | "theme" | "overlay" | "finish";
const STEPS: Step[] = ["welcome", "nick", "theme", "overlay", "finish"];

const SAMPLE_HOURS = [1, 1, 1, 1, 1, 1, 3, 6, 5, 8, 6, 7, 3, 11, 7, 8, 6, 8, 4, 1, 1, 1, 1, 1];

function ClockArt() {
  return (
    <svg className="ob-clock" viewBox="0 0 128 128" aria-hidden="true">
      <defs>
        <clipPath id="ob-clock-clip">
          <circle cx="64" cy="64" r="56" />
        </clipPath>
      </defs>
      <circle cx="64" cy="64" r="62" style={{ fill: "rgb(var(--accent-rgb) / 0.35)" }} />
      <g clipPath="url(#ob-clock-clip)">
        <rect x="0" y="0" width="128" height="128" style={{ fill: "rgb(var(--accent-rgb) / 0.55)" }} />
        {[14, 30, 44, 58, 74, 90, 104].map((x, index) => (
          <rect
            key={x}
            x={x}
            y="0"
            width={index % 2 === 0 ? 9 : 5}
            height="128"
            style={{ fill: "rgb(var(--accent-rgb) / 0.9)" }}
            opacity={index % 2 === 0 ? 0.55 : 0.35}
          />
        ))}
      </g>
      <path d="M64 64V22" stroke="#fff" strokeWidth="4" strokeLinecap="round" />
      <path d="M64 64l24 9" stroke="#fff" strokeWidth="4" strokeLinecap="round" />
      <circle cx="64" cy="64" r="4.5" fill="#fff" />
    </svg>
  );
}

function Dots({ step }: { step: Step }) {
  return (
    <div className="ob-dots" aria-hidden="true">
      {STEPS.map((entry) => (
        <span key={entry} className={entry === step ? "is-active" : ""} />
      ))}
    </div>
  );
}

export function Onboarding() {
  const { prefs, settings, update } = usePrefs();
  const [step, setStep] = useState<Step>("welcome");
  const [nickname, setNickname] = useState(prefs.nickname);
  const [triedOverlay, setTriedOverlay] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const index = STEPS.indexOf(step);
  const go = (next: Step) => setStep(next);
  const back = () => setStep(STEPS[Math.max(0, index - 1)]);
  const next = () => setStep(STEPS[Math.min(STEPS.length - 1, index + 1)]);

  useEffect(() => {
    if (step === "nick") {
      window.setTimeout(() => inputRef.current?.focus(), 60);
    }
  }, [step]);

  useEffect(() => {
    let dispose: (() => void) | undefined;
    let cancelled = false;
    void listen("overlay-shown", () => setTriedOverlay(true)).then((unlisten) => {
      if (cancelled) unlisten();
      else dispose = unlisten;
    });
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, []);

  const finish = () => update({ onboardingDone: true });
  const skip = () => update({ onboardingDone: true });

  const commitNickname = () => {
    update({ nickname: nickname.trim() });
    next();
  };

  const name = nickname.trim();
  const hotkey = shortcutParts(settings?.hotkey ?? "control+shift+Space");

  return (
    <div className="ob" key={step}>
      {step === "welcome" ? (
        <div className="ob__panel">
          <ClockArt />
          <h1 className="ob__title display">Make time for what matters.</h1>
          <p className="ob__sub">Let&rsquo;s get you set up.</p>
          <div className="ob__actions">
            <button className="btn btn--primary" type="button" onClick={next} autoFocus>
              Start setup <Icon name="arrowRight" size={18} />
            </button>
            <button className="btn btn--ghost btn--lg dim" type="button" onClick={skip}>
              Skip setup
            </button>
          </div>
        </div>
      ) : null}

      {step === "nick" ? (
        <div className="ob__panel">
          <h1 className="ob__title display">What should we call you?</h1>
          <p className="ob__sub">You can change this anytime.</p>

          <label className="ob-input">
            <Icon name="textCursor" size={18} />
            <input
              ref={inputRef}
              value={nickname}
              maxLength={24}
              placeholder="Enter your nickname"
              onChange={(event) => setNickname(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") commitNickname();
              }}
            />
          </label>

          {name ? (
            <div className="ob-preview ob-preview--greeting">
              <div className="display ob-preview__hello">
                {greetingFor()}, {name} <span aria-hidden="true">👋</span>
              </div>
              <div className="ob-preview__sub">Here&rsquo;s what you did today</div>
              <div className="ob-preview__lines">
                <span />
                <span />
              </div>
            </div>
          ) : null}

          <div className="ob__actions">
            <button className="btn btn--ghost btn--lg dim" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn--primary" type="button" onClick={commitNickname}>
              Continue <Icon name="arrowRight" size={18} />
            </button>
          </div>
        </div>
      ) : null}

      {step === "theme" ? (
        <div className="ob__panel ob__panel--wide">
          <h1 className="ob__title display">Make it yours.</h1>
          <p className="ob__sub">Customise the theme, colors, and style of the app.</p>

          <div className="ob-preview ob-preview--chart">
            <div className="card__title">Today&rsquo;s Screen Time</div>
            <div className="stat-big">
              4hr 12m<sup>-12%</sup>
            </div>
            <div className="ob-preview__chart">
              <BarChart values={SAMPLE_HOURS} labels={HOUR_LABELS} labelEvery={2} height={96} />
            </div>
          </div>

          <div className="swatches" role="radiogroup" aria-label="Theme">
            {THEMES.map((theme) => (
              <button
                key={theme.id}
                type="button"
                role="radio"
                aria-checked={prefs.theme === theme.id}
                aria-label={theme.label}
                title={theme.label}
                className={`swatch${prefs.theme === theme.id ? " is-active" : ""}`}
                style={{ background: theme.swatch }}
                onClick={() => update({ theme: theme.id })}
              />
            ))}
          </div>

          <div className="ob-slider">
            <span>Solid</span>
            <input
              className="slider"
              type="range"
              min={0}
              max={100}
              value={prefs.sidebarTranslucency}
              onChange={(event) => update({ sidebarTranslucency: Number(event.target.value) })}
              aria-label="Sidebar translucency"
            />
            <span>Glassy</span>
          </div>
          <p className="faint ob__hint">Sidebar translucency</p>

          <div className="ob__actions">
            <button className="btn btn--ghost btn--lg dim" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn--primary" type="button" onClick={next}>
              Continue <Icon name="arrowRight" size={18} />
            </button>
          </div>
        </div>
      ) : null}

      {step === "overlay" ? (
        <div className="ob__panel">
          <h1 className="ob__title display">Meet your quick overlay.</h1>
          <p className="ob__sub ob__sub--wide">
            The overlay contains all utilities you need without needing to open the app.
          </p>

          <div className="ob-keys">
            {hotkey.map((part, i) => (
              <span key={part} className="ob-keys__item">
                {i > 0 ? <span className="ob-keys__plus">+</span> : null}
                <span className="kbd kbd--lg">{part}</span>
              </span>
            ))}
          </div>
          <p className="dim">Press to open.</p>

          <div className="ob__actions">
            <button className="btn btn--ghost btn--lg dim" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn--accent btn--lg" type="button" onClick={() => void call("show_overlay")}>
              Try opening the overlay
            </button>
            <button
              className={`btn btn--lg ${triedOverlay ? "btn--primary" : ""}`}
              type="button"
              onClick={next}
            >
              Continue <Icon name="arrowRight" size={18} />
            </button>
          </div>
        </div>
      ) : null}

      {step === "finish" ? (
        <div className="ob__panel">
          <h1 className="ob__title display">Now, let&rsquo;s track your day.</h1>
          <p className="ob__sub ob__sub--wide">
            TrackMLN records how you spend your time, what tasks you completed, how much time you focused, and more
            throughout the day.
          </p>
          <p className="ob__sub ob__sub--wide">
            You can always check your statistics from the overlay or the main app.
          </p>
          <div className="ob__actions">
            <button className="btn btn--ghost btn--lg dim" type="button" onClick={() => go("overlay")}>
              Back
            </button>
            <button className="btn btn--accent btn--lg" type="button" onClick={finish} autoFocus>
              Start TrackMLN <Icon name="arrowRight" size={18} />
            </button>
          </div>
        </div>
      ) : null}

      <Dots step={step} />
    </div>
  );
}
