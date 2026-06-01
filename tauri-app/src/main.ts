import { createApp } from 'vue'
import App from './App.vue'
import DashboardView from './views/dashboard/index.vue'
import './styles/main.css'

const isDashboard = window.location.hash === '#/dashboard'
createApp(isDashboard ? DashboardView : App).mount('#app')
