import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Home } from "./pages/Home";
import { Testing } from "./pages/Testing";
import MessageCardDemo from "./pages/MessageCardDemo";
import { Chat } from "./pages/Chat";
import BranchDemo from "./pages/BranchDemo";

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/chat" element={<Chat />} />
        <Route path="/testing" element={<Testing />} />
        <Route path="/message-card" element={<MessageCardDemo />} />
        <Route path="/message-demo" element={<MessageCardDemo />} />
        <Route path="/branch-demo" element={<BranchDemo />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
