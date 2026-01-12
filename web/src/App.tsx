import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Home } from "./pages/Home";
import { Testing } from "./pages/Testing";
import MessageCardDemo from "./pages/MessageCardDemo";
import { Chat } from "./pages/Chat";

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/chat" element={<Chat />} />
        <Route path="/testing" element={<Testing />} />
        <Route path="/message-card" element={<MessageCardDemo />} />
        <Route path="/message-demo" element={<MessageCardDemo />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
