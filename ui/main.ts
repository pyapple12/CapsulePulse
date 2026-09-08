import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

// 应用入口：挂载 Vue 根组件（展示层骨架，业务逻辑零含量）
createApp(App).mount("#app");
