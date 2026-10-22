import { Component, type ErrorInfo, type FunctionComponent, type ReactNode } from "react";
import { logger } from "../core/log";
import "./crash.css";

const log = logger("crash");

/** Shows what threw while rendering its children, in place of them, so one broken view
 * doesn't blank the window. */
export class Crash extends Component<{ children: ReactNode }, { error?: Error }> {
  state: { error?: Error } = {};

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    log.error(error, info.componentStack ?? "");
  }

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="crash">
        <b>{error.message || String(error)}</b>
        <pre>{error.stack}</pre>
        <button type="button" onClick={() => this.setState({ error: undefined })}>
          Retry
        </button>
      </div>
    );
  }
}

/** `View` inside its own `Crash`. */
export function guarded<P extends object>(View: FunctionComponent<P>): FunctionComponent<P> {
  return (props: P) => (
    <Crash>
      <View {...props} />
    </Crash>
  );
}
