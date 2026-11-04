import { BrowserRouter, Route, Routes } from "react-router-dom";
import { Layout } from "./components/Layout";
import { Landing } from "./pages/Landing";
import { DatasetPage } from "./pages/DatasetPage";
import { ReportPage } from "./pages/ReportPage";
import { DrillPage } from "./pages/DrillPage";
import { HowPage } from "./pages/HowPage";
import { NotFound } from "./pages/NotFound";

export default function App() {
  return (
    <BrowserRouter>
      <Layout>
        <Routes>
          <Route path="/" element={<Landing />} />
          <Route path="/d/:id" element={<DatasetPage />} />
          <Route path="/r/:id" element={<ReportPage />} />
          <Route path="/drill" element={<DrillPage />} />
          <Route path="/how" element={<HowPage />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}
