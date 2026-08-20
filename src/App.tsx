import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import logoMark from '@/assets/logo-mark.svg'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader } from '@/components/ui/card'

async function testNotification() {
  let granted = await isPermissionGranted()
  if (!granted) {
    granted = (await requestPermission()) === 'granted'
  }
  if (granted) {
    sendNotification({ title: 'Overnight', body: 'Notifications are working.' })
  }
}

function App() {
  return (
    <div className="flex min-h-screen flex-col bg-background">
      <header className="flex items-center gap-2 border-b px-4 py-3">
        <img src={logoMark} alt="" className="h-5 w-auto" />
        <span className="text-lg font-semibold">overnight</span>
      </header>
      <main className="flex flex-1 items-center justify-center">
        <Card className="w-96">
          <CardHeader>
            <CardDescription>Desktop shell scaffold</CardDescription>
          </CardHeader>
          <CardContent>
            <Button onClick={testNotification}>Send test notification</Button>
          </CardContent>
        </Card>
      </main>
    </div>
  )
}

export default App
