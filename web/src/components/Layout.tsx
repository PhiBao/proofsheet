import { useEffect, useState } from "react";
import { Link, NavLink, useLocation } from "react-router-dom";
import { api } from "../lib/api";
import type { Health } from "../lib/types";
import { BrandMark } from "./BrandMark";

export function Layout({ children }: { children: React.ReactNode }) {
  const [health, setHealth] = useState<Health | null>(null);
  const location = useLocation();

  useEffect(() => {
    api.health().then(setHealth).catch(() => setHealth(null));
  }, [location.pathname]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="topbar-inner">
          <Link to="/" className="brand" aria-label="Proofsheet home">
            <BrandMark />
            <span>Proofsheet</span>
          </Link>
          <nav className="nav">
            <NavLink to="/how" className={({ isActive }) => (isActive ? "active" : "")}>
              How it works
            </NavLink>
            <NavLink to="/drill" className={({ isActive }) => (isActive ? "active hide-mobile" : "hide-mobile")}>
              Containment
            </NavLink>
            {health && (
              <span className="sandbox-pill" title={health.sandbox.detail}>
                <span className={`dot ${health.sandbox.ready ? "" : "warn"}`} />
                <span>
                  {health.sandbox.ready ? health.sandbox.isolation : "sandbox offline"}
                </span>
                <span className="pill-detail">· {health.region}</span>
              </span>
            )}
          </nav>
        </div>
      </header>
      <main>{children}</main>
      <footer className="footer">
        <div className="footer-inner">
          <span>Proofsheet — the analyst that shows its work.</span>
          <span>
            Vultr Agent Rush · Serverless Inference + sandboxed execution ·{" "}
            <Link to="/how" className="link">
              architecture
            </Link>
          </span>
        </div>
      </footer>
    </div>
  );
}
