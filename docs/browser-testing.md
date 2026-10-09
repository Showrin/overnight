# Browser testing with Claude in Chrome

A sandbox can't open a browser. With browser testing on, the sandbox agent writes a test plan and hands it to Overnight. A Claude agent on your machine runs the plan in a dedicated Chrome and sends a report back, which the sandbox agent then acts on.

## One-time setup

1. Open **Settings → Browser Testing** and click **Set up**. A separate Chrome window opens with its own profile.
2. In that window, install the Claude extension and sign in to claude.ai with the same account Claude Code uses on this machine.
3. Sign in to any test accounts your app needs. They stay in this profile only.
4. Remove or turn off the Claude extension in your everyday Chrome profile, so Claude can only connect to the testing one.

## Turn it on for a sandbox

1. Open the sandbox and go to the **Browser** tab.
2. Choose **What Chrome opens**:
   - **App in this sandbox**: the dev server the agent runs inside the sandbox. Set the port it listens on (default `8080`).
   - **Server on this machine**: servers you run yourself in the project folder. Enter their URL, e.g. `http://localhost:3000`.
3. Click **Save**, then switch **Claude in Chrome** on.

The sandbox must be running for the switch to take effect. If it's stopped, it's set up the next time it starts.

## Run a test

1. Ask the sandbox agent to browser-test its work, e.g. *"Test the new login form in the browser."* It uses its `browser-test` skill to write a test plan and submit it.
2. For **Server on this machine**, keep your servers running. Before each test, Overnight pulls the sandbox's branch into the project folder, checking it out first if you're on another branch. If the URL isn't responding, you get a notification to start your servers, and the test begins once it responds.
3. Watch it in the **Browser** tab under **Test runs**:
   - **Sandbox handoff**: the plan the sandbox agent sent.
   - **Host report**: the verdict, every check, the issues found and suggestions.
4. The sandbox agent receives the same report, fixes the issues, and can test again.

Click **Cancel** to stop a queued or running test.

## If something goes wrong

| Error | Fix |
| - | - |
| "Claude in Chrome is turned off for this sandbox" | Turn it on in the sandbox's **Browser** tab. |
| "the testing browser isn't set up yet" | Do the one-time setup above. |
| "has uncommitted changes" | Commit or stash your changes in the project folder, then test again. |
| "couldn't pull … your uncommitted changes may conflict" | One of your edits touches a file the sandbox changed. Commit or stash it. |
| "has diverged from the sandbox's" | Merge your local branch with the sandbox's branch by hand. |
| "the host agent failed: … not connected" | Open the testing Chrome from **Settings → Browser Testing** and check the extension is signed in. |
| "didn't respond within …" | The servers never came up at the URL. Check the URL and your servers. |
