export function App() {
  return (
    <main id="main-content" className="foundation-shell">
      <p className="eyebrow">JobGlass</p>
      <h1>See what runs next.</h1>
      <p className="lede">
        This build contains the verified desktop foundation. Native scheduler
        evidence is added in the next tested slice.
      </p>
      <section className="empty-state" aria-labelledby="empty-title">
        <h2 id="empty-title">No scheduler evidence loaded</h2>
        <p>
          JobGlass stays read-only and reports only evidence visible to the
          current user.
        </p>
      </section>
    </main>
  );
}
