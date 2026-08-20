import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { Titlebar } from '@/components/titlebar'
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
    <div className="flex h-screen flex-col bg-background">
      <Titlebar />
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
