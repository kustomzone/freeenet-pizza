use dioxus::prelude::*;

#[component]
pub fn AboutPage() -> Element {
    rsx! {
        div {
            class: "about-page",
            div {
                class: "about-header",
                h1 { "About Pizza Order Manager" }
            }

            div {
                class: "about-content",
                section {
                    class: "about-section",
                    h2 { "What is Pizza Order Manager?" }
                    p {
                        "Pizza Order Manager is a decentralized application for coordinating group purchases. "
                        "While we use pizza as our example (because who doesn't love pizza?), this app "
                        "works for any kind of group order - whether you're organizing lunch for the office, "
                        "collecting coffee orders, arranging a group gift, or coordinating any shared purchase."
                    }
                }

                section {
                    class: "about-section",
                    h2 { "How it Works" }
                    ul {
                        li {
                            strong { "Create an Order: " }
                            "Start a new group order with a name and currency. Share the link with your group."
                        }
                        li {
                            strong { "Add Items: " }
                            "Each participant adds their own order with their name, what they want, and how much it costs."
                        }
                        li {
                            strong { "Track Payments: " }
                            "The order creator (admin) can mark items as paid to keep track of who has settled up."
                        }
                        li {
                            strong { "Decentralized: " }
                            "Your orders are stored on the Freenet network - no central server, no accounts needed."
                        }
                    }
                }

                section {
                    class: "about-section",
                    h2 { "Built on Freenet" }
                    p {
                        "This application runs on "
                        a {
                            href: "https://freenet.org",
                            target: "_blank",
                            "Freenet"
                        }
                        ", a decentralized platform for censorship-resistant communication and applications. "
                        "Your data is distributed across the network, not stored on any single server."
                    }
                }

                section {
                    class: "about-section",
                    h2 { "Use Cases" }
                    div {
                        class: "use-cases-grid",
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "🍕" }
                            span { "Pizza nights" }
                        }
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "☕" }
                            span { "Coffee runs" }
                        }
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "🍱" }
                            span { "Lunch orders" }
                        }
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "🎁" }
                            span { "Group gifts" }
                        }
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "🛒" }
                            span { "Bulk purchases" }
                        }
                        div {
                            class: "use-case",
                            span { class: "use-case-icon", "🎉" }
                            span { "Event supplies" }
                        }
                    }
                }
            }
        }
    }
}
