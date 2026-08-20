import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

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
    <main className="flex min-h-screen items-center justify-center bg-background">
      <Card className="w-96">
        <CardHeader>
          <CardTitle>Overnight</CardTitle>
          <CardDescription>Desktop shell scaffold</CardDescription>
        </CardHeader>
        <CardContent>
          <Button onClick={testNotification}>Send test notification</Button>
        </CardContent>
      </Card>
    </main>
  )
}

export default App
