import { useState } from "react";
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
  const [pendingAction, setPendingAction] =
    useState<ProviderNavigationAction | null>(null);

  if (!navigation.available) return null;

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

  const providerLabel = getProviderLabel(provider);

  return (
    <nav
      aria-label={`${providerLabel} browser controls`}
      className="browser-navigation-capsule"
    >
      <button
        aria-disabled={backDisabled}
        aria-label="Go back"
        className="browser-navigation-button"
        onClick={() => void runAction("back", backDisabled)}
        title={backTitle}
        type="button"
      >
        <Icon name="back" size={17} />
      </button>
      <button
        aria-disabled={forwardDisabled}
        aria-label="Go forward"
        className="browser-navigation-button"
        onClick={() => void runAction("forward", forwardDisabled)}
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
        title={lockedTitle ?? reloadLabel}
        type="button"
      >
        <Icon name={navigation.isLoading ? "stop" : "reload"} size={17} />
      </button>
    </nav>
  );
}
