import {
  useEffect,
  useRef,
  useState,
  type FocusEvent,
  type KeyboardEvent,
} from "react";
import { Icon } from "../../shared/Icon";
import { publishNotice } from "../../shared/notices";
import { normalizeProviderError, providerGateway } from "./gateway";
import {
  getProviderLabel,
  type Provider,
  type ProviderNavigationAction,
  type ProviderNavigationState,
} from "./model";
import { useProviderStore } from "./store";

const COARSE_POINTER_QUERY =
  "(hover: none), (pointer: coarse), (any-pointer: coarse)";

function useAlwaysExpandedControls(): boolean {
  const [alwaysExpanded, setAlwaysExpanded] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia?.(COARSE_POINTER_QUERY).matches === true,
  );

  useEffect(() => {
    if (!window.matchMedia) return;

    const media = window.matchMedia(COARSE_POINTER_QUERY);
    const update = () => setAlwaysExpanded(media.matches);
    update();

    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", update);
      return () => media.removeEventListener("change", update);
    }

    media.addListener(update);
    return () => media.removeListener(update);
  }, []);

  return alwaysExpanded;
}

type ProviderNavigationCapsuleProps = {
  isPlacing: boolean;
  navigation: ProviderNavigationState;
  provider: Provider;
};

export function ProviderNavigationCapsule({
  isPlacing,
  navigation,
  provider,
}: ProviderNavigationCapsuleProps) {
  const alwaysExpanded = useAlwaysExpandedControls();
  const [pointerInside, setPointerInside] = useState(false);
  const [focusInside, setFocusInside] = useState(false);
  const [pendingAction, setPendingAction] =
    useState<ProviderNavigationAction | null>(null);
  const suppressFocusExpansionRef = useRef(false);
  const backButtonRef = useRef<HTMLButtonElement>(null);

  if (!navigation.available) return null;

  const expanded = alwaysExpanded || pointerInside || focusInside;
  const actionInFlight = pendingAction !== null;
  const controlsLocked = isPlacing || actionInFlight;
  const backDisabled = controlsLocked || !navigation.canGoBack;
  const forwardDisabled = controlsLocked || !navigation.canGoForward;
  const reloadAction = navigation.isLoading ? "stop" : "reload";
  const reloadLabel = navigation.isLoading
    ? "Stop loading page"
    : "Reload page";
  const lockedTitle = isPlacing
    ? "Wait for prompt placement to finish"
    : actionInFlight
      ? "Wait for the browser action to finish"
      : null;
  const backTitle =
    lockedTitle ??
    (navigation.canGoBack ? "Go back" : "No page to go back to");
  const forwardTitle =
    lockedTitle ??
    (navigation.canGoForward
      ? "Go forward"
      : "No page to go forward to");

  const runAction = async (
    action: ProviderNavigationAction,
    disabled: boolean,
  ) => {
    if (disabled || pendingAction !== null) return;

    setPendingAction(action);
    try {
      const acknowledgedNavigation = await providerGateway.controlNavigation(
        provider,
        navigation.generation,
        action,
      );
      useProviderStore
        .getState()
        .updateNavigationState(acknowledgedNavigation);
    } catch (error) {
      publishNotice("error", normalizeProviderError(error).message);
    } finally {
      setPendingAction((current) => (current === action ? null : current));
    }
  };

  const handleBlur = (event: FocusEvent<HTMLElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget)) {
      setFocusInside(false);
    }
  };

  const handlePointerEnter = () => {
    setPointerInside(true);
  };

  const handlePointerLeave = () => {
    setPointerInside(false);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Escape" || alwaysExpanded) return;

    event.preventDefault();
    suppressFocusExpansionRef.current = true;
    setPointerInside(false);
    setFocusInside(false);
    backButtonRef.current?.focus({ preventScroll: true });
    queueMicrotask(() => {
      suppressFocusExpansionRef.current = false;
    });
  };

  const providerLabel = getProviderLabel(provider);

  return (
    <nav
      aria-label={`${providerLabel} browser controls`}
      className="browser-navigation-capsule"
      data-expanded={expanded}
      onBlurCapture={handleBlur}
      onFocusCapture={() => {
        if (!suppressFocusExpansionRef.current) setFocusInside(true);
      }}
      onKeyDown={handleKeyDown}
      onPointerEnter={handlePointerEnter}
      onPointerLeave={handlePointerLeave}
    >
      <button
        aria-disabled={backDisabled}
        aria-label="Go back"
        className="browser-navigation-button"
        onClick={() => void runAction("back", backDisabled)}
        ref={backButtonRef}
        title={backTitle}
        type="button"
      >
        <Icon name="back" size={17} />
      </button>

      <span aria-hidden={!expanded} className="browser-navigation-extras">
        <button
          aria-disabled={forwardDisabled}
          aria-label="Go forward"
          className="browser-navigation-button"
          onClick={() => void runAction("forward", forwardDisabled)}
          tabIndex={expanded ? 0 : -1}
          title={forwardTitle}
          type="button"
        >
          <Icon name="forward" size={17} />
        </button>
        <button
          aria-disabled={controlsLocked}
          aria-label={reloadLabel}
          className="browser-navigation-button"
          onClick={() => void runAction(reloadAction, controlsLocked)}
          tabIndex={expanded ? 0 : -1}
          title={lockedTitle ?? reloadLabel}
          type="button"
        >
          <Icon
            name={navigation.isLoading ? "stop" : "reload"}
            size={17}
          />
        </button>
      </span>
    </nav>
  );
}
